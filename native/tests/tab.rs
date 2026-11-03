//! The tab's behaviour, `test/tab.test.js` ported: the Node tests drive the controller and the view
//! over the real script; these drive the module's own runtime through the SDK's harness, over
//! what the real script printed (`fixtures/answers.json`).

mod support;

use serde_json::{Value, json};

use hangar_kit_sdk::api::KitError;
use hangar_kit_sdk::testing::{self, StartExt};
use hangar_kit_sdk::ui::{Element, Sort};
use kit_db_tab::logic::Direction;
use kit_db_tab::{DetailPhase, Full, Phase, Screen};
use support::*;

const SHOW_ALL: &str = "Show all 6,000 characters";

// ---------------------------------------------------------------- Opening.

#[test]
fn opens_the_only_database_found_lists_its_tables_and_remembers_it() {
    let (tab, host) = started("single");
    let db = tab.kit();
    assert_eq!(db.screen(), Screen::Ready);
    assert_eq!(db.path(), Some("app.db"));
    let database = db.database().unwrap();
    assert_eq!(database.phase, Phase::Ready);
    let events = database.tables.iter().find(|t| t.name == "events").unwrap();
    assert_eq!(events.rows, 120);
    assert!(
        !host.data.contains_key("state.json"),
        "opening what was found is not a choice to remember"
    );
    for needle in [
        "Tables",
        "events",
        "120 rows",
        "Read-only",
        "Values may be secrets",
    ] {
        assert!(shows(&tab, needle), "{needle}: {}", text(&tab));
    }
    assert_eq!(host.title.as_deref(), Some("Database"));
}

#[test]
fn an_agents_params_path_open_that_database_and_a_new_ask_replaces_it() {
    let mut host = Host::new("pair").with_agents();
    let mut tab = open(
        &mut host,
        start().param("path", "other.sqlite").caller("Jack"),
    );
    assert_eq!(tab.kit().path(), Some("other.sqlite"));
    assert_eq!(tab.kit().files().len(), 2);
    assert!(
        !host.data.contains_key("state.json"),
        "the params of the start are not a pick"
    );

    // The event's data is `{ params, caller }` itself. The page read `event.data.params.path` and
    // so never heard a singleton's new path; this tab does.
    tab.event(
        "tab.params",
        json!({ "params": { "path": "app.db" }, "caller": { "pane": "Jack" } }),
    );
    host.serve(&mut tab);
    assert_eq!(tab.kit().path(), Some("app.db"));
    assert_eq!(host.data["state.json"], r#"{"path":"app.db"}"#);

    tab.event(
        "tab.params",
        json!({ "params": { "path": "other.sqlite" }, "caller": null }),
    );
    host.serve(&mut tab);
    assert_eq!(tab.kit().path(), Some("other.sqlite"));
    assert_eq!(host.data["state.json"], r#"{"path":"other.sqlite"}"#);
}

#[test]
fn the_tab_listens_for_the_params_event() {
    let (tab, _) = started("single");
    let events = &tab.subscriptions().events;
    for name in ["tab.params", "panes.changed", "theme"] {
        assert!(events.contains(&name.to_owned()), "{name}: {events:?}");
    }
}

#[test]
fn a_path_that_breaks_the_patterns_is_not_asked_for() {
    let mut host = Host::new("single");
    let mut tab = open(&mut host, start().param("path", "/etc/passwd"));
    assert_eq!(
        tab.kit().path(),
        Some("app.db"),
        "the first database, as if none was asked"
    );
    let long = "x".repeat(300);
    for bad in ["-rf", "a;b.db", "", long.as_str()] {
        tab.event(
            "tab.params",
            json!({ "params": { "path": bad }, "caller": null }),
        );
        assert!(tab.calls().is_empty(), "{bad:?} calls nothing");
    }
    tab.event("tab.params", json!({ "params": {}, "caller": null }));
    tab.event("tab.params", json!("nonsense"));
    assert!(tab.calls().is_empty());
    assert_eq!(tab.kit().path(), Some("app.db"));
}

#[test]
fn an_ask_while_the_tab_starts_is_not_lost() {
    let mut host = Host::new("pair").with_agents();
    let mut tab = hangar_kit_sdk::testing::Harness::<kit_db_tab::Db>::new(start());
    tab.event(
        "tab.params",
        json!({ "params": { "path": "other.sqlite" }, "caller": null }),
    );
    host.serve(&mut tab);
    assert_eq!(tab.kit().path(), Some("other.sqlite"));
}

#[test]
fn a_remembered_database_is_opened_and_one_that_is_gone_falls_back_to_the_first_found() {
    let mut host = Host::new("pair").remembering("other.sqlite");
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().path(), Some("other.sqlite"));

    let mut host = Host::new("single").remembering("gone.db");
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().path(), Some("app.db"));

    for garbage in ["not json", "[]", r#"{"path":7}"#, r#"{"path":"../x"}"#] {
        let mut host = Host::new("single");
        host.data.insert("state.json".into(), garbage.into());
        let tab = open(&mut host, start());
        assert_eq!(tab.kit().path(), Some("app.db"), "{garbage}");
    }
}

#[test]
fn picking_a_database_remembers_it_and_picking_the_one_open_does_nothing() {
    let mut host = Host::new("pair").with_agents();
    let mut tab = open(&mut host, start());
    assert_eq!(tab.kit().path(), Some("app.db"));
    assert_eq!(chosen(&tab, "Database").as_deref(), Some("app.db"));
    tab.choose("Database", "app.db");
    assert!(
        tab.calls().is_empty(),
        "the database already open is not opened again"
    );
    tab.choose("Database", "other.sqlite");
    host.serve(&mut tab);
    assert_eq!(tab.kit().path(), Some("other.sqlite"));
    assert_eq!(host.data["state.json"], r#"{"path":"other.sqlite"}"#);
}

#[test]
fn files_the_commands_cannot_take_are_listed_and_cannot_be_picked() {
    let (tab, _) = started("odd");
    let listed = options(&tab, "Database");
    assert_eq!(listed.len(), 2);
    let find = |value: &str| {
        listed
            .iter()
            .find(|option| option.0 == value)
            .unwrap()
            .clone()
    };
    assert_eq!(find("app.db"), ("app.db".into(), "app.db".into(), false));
    let odd = find("bad ;name.db");
    assert!(odd.2, "disabled");
    assert!(
        odd.1.starts_with("bad ;name.db — Not openable"),
        "{}",
        odd.1
    );
    assert_eq!(
        tab.kit().path(),
        Some("app.db"),
        "the first that can be opened"
    );
}

// ---------------------------------------------------------------- A table.

#[test]
fn a_table_its_structure_a_page_of_rows_sorting_that_cycles_paging_a_long_value_on_demand() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "events");
    let detail = tab.kit().detail().unwrap().clone();
    assert_eq!(detail.phase, DetailPhase::Ready);
    let schema = detail.schema.unwrap();
    let names: Vec<&str> = schema.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "kind", "n"]);
    let page = detail.page.unwrap();
    assert_eq!((page.rows.len(), page.total), (50, 120));
    assert!(shows(&tab, "1–50 of 120"));

    // A press on a header sorts ascending, the next descending, the next not at all.
    sort(&mut tab, &mut host, 1);
    assert_eq!(tab.kit().sorted(), (2, Direction::Asc));
    assert_eq!(grid(&tab).unwrap().sort, Some(Sort::ascending(1)));
    sort(&mut tab, &mut host, 1);
    assert_eq!(tab.kit().sorted(), (2, Direction::Desc));
    assert_eq!(grid(&tab).unwrap().sort, Some(Sort::descending(1)));
    let kinds: Vec<String> = grid(&tab)
        .unwrap()
        .rows
        .iter()
        .map(|row| row[1].clone())
        .collect();
    let mut expected = kinds.clone();
    expected.sort();
    expected.reverse();
    assert_eq!(kinds, expected);
    sort(&mut tab, &mut host, 1);
    assert_eq!(tab.kit().sorted().0, 0);
    assert_eq!(
        grid(&tab).unwrap().sort,
        None,
        "the third press takes the sort away"
    );
    let rows = host.commands("rows");
    let sorts: Vec<_> = rows
        .iter()
        .map(|args| {
            (
                args["sort"].as_str(),
                args["direction"].as_str(),
                args["offset"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        sorts,
        [
            ("0", "asc", "0"),
            ("2", "asc", "0"),
            ("2", "desc", "0"),
            ("0", "asc", "0")
        ]
    );

    // Another column starts again, ascending.
    sort(&mut tab, &mut host, 2);
    sort(&mut tab, &mut host, 0);
    assert_eq!(tab.kit().sorted(), (1, Direction::Asc));
    sort(&mut tab, &mut host, 1);
    assert_eq!(tab.kit().sorted(), (2, Direction::Asc));
    sort(&mut tab, &mut host, 1);
    sort(&mut tab, &mut host, 1);

    // Paging.
    press(&mut tab, &mut host, "Next");
    press(&mut tab, &mut host, "Next");
    assert_eq!(tab.kit().offset(), 100);
    assert_eq!(
        tab.kit()
            .detail()
            .unwrap()
            .page
            .as_ref()
            .unwrap()
            .rows
            .len(),
        20
    );
    assert!(shows(&tab, "101–120 of 120"));
    assert_eq!(buttons(&tab, "Next"), [false], "no page after the last");
    assert_eq!(buttons(&tab, "Previous"), [true]);
    press(&mut tab, &mut host, "Previous");
    assert_eq!(tab.kit().offset(), 50);
    press(&mut tab, &mut host, "Previous");
    assert_eq!(tab.kit().offset(), 0);
    assert_eq!(buttons(&tab, "Previous"), [false]);
}

#[test]
fn a_press_on_a_row_offers_the_whole_of_a_cut_value_and_a_second_one_folds_it() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    let view = text(&tab);
    assert!(view.contains('…'), "a cut value ends in an ellipsis");
    assert!(buttons(&tab, SHOW_ALL).is_empty(), "no row is pressed yet");
    press_row(&mut tab, 0);
    assert_eq!(
        buttons(&tab, SHOW_ALL),
        [true],
        "a cut value offers the whole one"
    );
    assert_eq!(grid(&tab).unwrap().selected, Some(0));

    press(&mut tab, &mut host, SHOW_ALL);
    let Some(Full::Done { value, cut }) = tab.kit().full(0, 5) else {
        panic!("the whole value was read: {:?}", tab.kit().full(0, 5));
    };
    assert_eq!(value.chars().count(), 6000);
    assert!(!cut);
    let asked = host.commands("rows").pop().unwrap();
    assert_eq!(
        (asked["limit"].as_str(), asked["chars"].as_str()),
        ("1", "100000")
    );
    assert_eq!(
        (asked["offset"].as_str(), asked["sort"].as_str()),
        ("0", "0")
    );
    assert_eq!(buttons(&tab, "Show less"), [true]);
    assert!(
        texts(&tab).iter().any(|shown| shown.len() == 6000),
        "the whole value is on the page"
    );

    press(&mut tab, &mut host, "Show less");
    assert!(tab.kit().full(0, 5).is_none());
    assert!(!texts(&tab).iter().any(|shown| shown.len() == 6000));
    assert_eq!(buttons(&tab, SHOW_ALL), [true]);
}

#[test]
fn the_whole_value_is_asked_for_by_its_place_in_the_sorted_page() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    sort(&mut tab, &mut host, 1);
    press_row(&mut tab, 0);
    press(&mut tab, &mut host, SHOW_ALL);
    let asked = host.commands("rows").pop().unwrap();
    assert_eq!(asked["sort"], "2");
    assert_eq!(asked["direction"], "asc");
    assert!(matches!(tab.kit().full(0, 5), Some(Full::Done { .. })));
    // A new page forgets what was opened.
    sort(&mut tab, &mut host, 1);
    assert!(tab.kit().full(0, 5).is_none());
    assert!(buttons(&tab, "Show less").is_empty());
}

#[test]
fn a_value_that_cannot_be_read_whole_says_so() {
    let mut host = Host::new("single").answering("rows", |args| {
        if args["limit"] == "1" {
            Ok(printed(1, "", "boom\n"))
        } else {
            Ok(recorded_for(args))
        }
    });
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "users");
    press_row(&mut tab, 0);
    press(&mut tab, &mut host, SHOW_ALL);
    assert_eq!(tab.kit().full(0, 5), Some(&Full::Failed));
    assert!(shows(&tab, "The whole value could not be read."));
    // It can be tried again.
    assert_eq!(buttons(&tab, SHOW_ALL), [true]);
}

#[test]
fn long_values_blobs_and_nulls_are_told_apart_on_screen() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    let english = text(&tab);
    assert!(english.contains("8 bytes (binary)"), "{english}");
    assert!(english.contains("NULL"));
    assert!(english.contains("Éloïse ‘quoted’"));
    let grid = grid(&tab).unwrap();
    assert_eq!(grid.rows[2][1], "Éloïse ‘quoted’");
    assert_eq!(grid.rows[0][8], "1");
    assert_eq!(grid.rows[0][7], "3.5");

    let mut host = Host::new("single").with_agents();
    let mut french = open(&mut host, start().locale("fr"));
    open_table(&mut french, &mut host, "users");
    let french = text(&french);
    assert!(french.contains("8 octets (binaire)"), "{french}");
    assert!(french.contains("Lecture seule"));
    assert!(french.contains("Confier à un agent"));
}

#[test]
fn numbers_are_aligned_to_the_end_and_texts_are_not() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    let mut aligns = Vec::new();
    tab.view().root.for_each(&mut |element| {
        if let Element::Table(table) = element {
            aligns = table
                .columns
                .iter()
                .map(|c| (c.title.clone(), c.sortable, c.align))
                .collect();
        }
    });
    use hangar_kit_sdk::ui::Align::{End, Start};
    let align = |name: &str| aligns.iter().find(|(title, _, _)| title == name).unwrap().2;
    assert_eq!(align("id"), End);
    assert_eq!(align("score"), End);
    assert_eq!(align("active"), End);
    assert_eq!(align("name"), Start);
    assert_eq!(align("avatar"), Start);
    assert!(
        aligns.iter().all(|(_, sortable, _)| *sortable),
        "every column sorts"
    );
}

#[test]
fn the_markup_of_a_value_is_text_never_parsed() {
    let document = json!({
        "ok": true, "table": "users",
        "columns": [{ "name": "<img src=x onerror=alert(1)>", "type": "", "primaryKey": false }],
        "rows": [[{ "type": "text", "value": "<script>alert(1)</script>", "length": 25, "cut": false }]],
        "offset": 0, "total": 1, "sort": 0, "direction": "asc", "capped": false
    })
    .to_string();
    let mut host =
        Host::new("single").answering("rows", move |_| Ok(printed(0, document.clone(), "")));
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "users");
    assert!(shows(&tab, "<script>alert(1)</script>"));
    let grid = grid(&tab).unwrap();
    assert_eq!(grid.columns, ["<img src=x onerror=alert(1)>"]);
    assert_eq!(grid.rows, [["<script>alert(1)</script>"]]);
}

#[test]
fn a_value_is_shown_on_one_line_and_its_tooltip_holds_the_start_of_it() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    let drawn = grid(&tab).unwrap();
    let bio = &drawn.rows[0][5];
    assert!(bio.ends_with('…') && bio.chars().count() <= 61, "{bio}");
    let tips: Vec<String> = texts(&tab)
        .into_iter()
        .filter(|t| t.starts_with("lorem ipsum"))
        .collect();
    assert!(
        tips.iter().any(|tip| tip.chars().count() == 200),
        "the tooltip holds the 200 characters the script sent"
    );

    let document = json!({
        "ok": true, "table": "users",
        "columns": [{ "name": "note", "type": "TEXT", "primaryKey": false }],
        "rows": [[{ "type": "text", "value": "line one\n\n   line\ttwo", "length": 21, "cut": false }]],
        "offset": 0, "total": 1, "sort": 0, "direction": "asc", "capped": false
    })
    .to_string();
    let mut host =
        Host::new("single").answering("rows", move |_| Ok(printed(0, document.clone(), "")));
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "users");
    assert_eq!(grid(&tab).unwrap().rows, [["line one line two"]]);
}

#[test]
fn the_structure_of_a_table_is_a_list_of_columns_and_chips_for_its_indexes() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    assert!(!shows(&tab, "users_email"), "the rows come first");
    tab.choose("Rows", "structure");
    assert_eq!(tab.kit().detail().unwrap().phase, DetailPhase::Ready);
    for needle in [
        "INTEGER · key",
        "TEXT · required",
        "TEXT · default 'none'",
        "BLOB",
        "Indexes",
        "users_email · unique",
        "users_name",
    ] {
        assert!(shows(&tab, needle), "{needle}: {}", text(&tab));
    }
    assert!(!shows(&tab, "users_name ·"), "only a unique index says so");
    // Back to the rows; the choice stays when another table is picked.
    tab.choose("Rows", "rows");
    assert!(grid(&tab).is_some());
    tab.choose("Rows", "structure");
    open_table(&mut tab, &mut host, "events");
    assert!(shows(&tab, "INTEGER · key"));
    assert!(grid(&tab).is_none());
}

#[test]
fn views_and_names_the_commands_cannot_take_are_told_in_the_list() {
    let (tab, _) = started("single");
    assert!(shows(&tab, "view · 2 columns"), "{}", text(&tab));
    assert!(shows(&tab, "1 column · Name not supported by this kit"));
    let mut odd = None;
    tab.view().root.for_each(&mut |element| {
        if let Element::List(list) = element {
            odd = odd.take().or(list
                .items
                .iter()
                .find(|item| item.title == "odd name")
                .map(|i| i.on_press));
        }
    });
    assert_eq!(odd, Some(None), "a name outside the pattern has no press");
    assert_eq!(tab.kit().table(), None);
}

#[test]
fn a_table_with_no_row_says_so() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "empty_one");
    assert!(shows(&tab, "This table has no row."));
    assert!(grid(&tab).is_none());
}

#[test]
fn a_database_of_one_table_opens_it() {
    let mut host = Host::new("single").answering("tables", |_| {
        Ok(printed(
            0,
            r#"{"ok":true,"driver":"node:sqlite","size":1,"tables":[{"name":"events","type":"table","rows":120,"columns":3,"viewable":true}]}"#,
            "",
        ))
    });
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().table(), Some("events"));
    assert!(grid(&tab).is_some());
}

#[test]
fn refreshing_lists_again_and_reads_the_open_table_again() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "events");
    sort(&mut tab, &mut host, 1);
    host.log.clear();
    press(&mut tab, &mut host, "Refresh");
    let methods: Vec<&str> = host.log.iter().map(|(method, _)| method.as_str()).collect();
    assert_eq!(methods[0], "command.run");
    assert_eq!(host.commands("list").len(), 1);
    assert_eq!(host.commands("tables").len(), 1);
    assert_eq!(tab.kit().path(), Some("app.db"));
    assert_eq!(
        tab.kit().table(),
        Some("events"),
        "the table is looked at again"
    );
    assert_eq!(
        tab.kit().sorted(),
        (0, Direction::Asc),
        "from the start of it"
    );
    assert!(grid(&tab).is_some());
    assert!(!host.data.contains_key("state.json"));
}

// ---------------------------------------------------------------- Handing a table over.

#[test]
fn hand_over_sends_the_table_and_the_question_and_no_value_of_any_row() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "users");
    sort(&mut tab, &mut host, 1);
    press_row(&mut tab, 0);
    press(&mut tab, &mut host, SHOW_ALL);
    assert!(
        host.sent.is_empty(),
        "nothing is sent by opening, picking, sorting or reading"
    );
    assert!(host.calls_to("panes.send").is_empty());

    tab.press("Hand over to an agent");
    tab.type_into("question", "Why is api_token filled for only one user?");
    press(&mut tab, &mut host, "Send");
    assert_eq!(host.sent.len(), 1);
    let (pane, message) = &host.sent[0];
    assert_eq!(pane, "Jack", "the orchestrator by default");
    assert!(message.contains("Database file: app.db"));
    assert!(message.contains("Table: users (3 rows)"));
    assert!(message.contains(
        "Columns: id INTEGER (primary key), name TEXT, email TEXT, api_token TEXT, password_hash TEXT, bio TEXT, avatar BLOB, score REAL, active INTEGER"
    ));
    assert!(message.contains("Question: Why is api_token filled for only one user?"));
    for secret in
        SECRETS
            .into_iter()
            .chain(["Bob", "Ada", "lorem ipsum", "bob@example.invalid", "Éloïse"])
    {
        assert!(
            !message.contains(secret),
            "{secret} did not go to the agent"
        );
    }
    assert_eq!(tab.kit().hand_over().sent.as_deref(), Some("Jack"));
    assert!(!tab.kit().hand_over().open);
    assert!(shows(&tab, "Handed to Jack"));
}

#[test]
fn no_value_of_a_row_leaves_the_screen_by_any_call_log_or_toast() {
    let (mut tab, mut host) = started("single");
    testing::take_logs();
    open_table(&mut tab, &mut host, "users");
    sort(&mut tab, &mut host, 1);
    press_row(&mut tab, 0);
    press(&mut tab, &mut host, SHOW_ALL);
    tab.press("Hand over to an agent");
    tab.type_into("question", "What is in there?");
    press(&mut tab, &mut host, "Send");
    tab.choose("Rows", "structure");
    press(&mut tab, &mut host, "Refresh");
    let sent_out =
        serde_json::to_string(&host.log).unwrap() + &format!("{:?}", testing::take_logs());
    for secret in SECRETS
        .into_iter()
        .chain(["bob@example.invalid", "lorem ipsum", "Éloïse"])
    {
        assert!(!sent_out.contains(secret), "{secret} left the screen");
    }
    let used: std::collections::BTreeSet<&str> = host.log.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        used.into_iter().collect::<Vec<_>>(),
        [
            "command.run",
            "data.read",
            "panes.list",
            "panes.send",
            "tab.setTitle"
        ],
        "the calls the tab makes, and no other"
    );
}

#[test]
fn the_question_is_required_the_agent_can_be_picked_and_a_refused_send_says_why() {
    let mut host = Host::new("single")
        .with_agents()
        .failing_sends("unavailable");
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    tab.press("Hand over to an agent");
    assert_eq!(buttons(&tab, "Send"), [false], "no question, no send");
    assert!(host.calls_to("panes.send").is_empty());
    assert_eq!(chosen(&tab, "Agent").as_deref(), Some("Jack"));
    let agents: Vec<String> = options(&tab, "Agent").into_iter().map(|o| o.0).collect();
    assert_eq!(agents, ["Jack", "Jude"], "a shell cannot be handed a table");

    tab.choose("Agent", "Jude");
    tab.type_into("question", "   ");
    assert_eq!(
        buttons(&tab, "Send"),
        [false],
        "a question of spaces is none"
    );
    tab.type_into("question", "Which kind is rarest?");
    assert_eq!(buttons(&tab, "Send"), [true]);
    press(&mut tab, &mut host, "Send");
    assert_eq!(host.calls_to("panes.send")[0]["pane"], "Jude");
    let problem = tab.kit().hand_over().problem.clone().unwrap();
    assert_eq!(problem.code, "unavailable");
    assert!(shows(&tab, "Jude cannot receive text now"));
    assert_eq!(
        tab.kit().hand_over().question,
        "Which kind is rarest?",
        "the question is kept for another try"
    );
    assert!(!tab.kit().hand_over().sending);
    assert!(tab.kit().hand_over().open);
    assert_eq!(
        host.calls_to("panes.list").len(),
        2,
        "who can be reached is asked again"
    );
}

#[test]
fn every_refusal_of_a_send_has_its_words() {
    for (code, words) in [
        ("denied", "may not send to agents"),
        ("not_found", "Jack is not in this workspace any more"),
        ("rate_limited", "Too many sends in a minute"),
        ("failed", "It could not be sent: could not: failed"),
        ("busy", "It could not be sent: could not: busy"),
    ] {
        let mut host = Host::new("single").with_agents().failing_sends(code);
        let mut tab = open(&mut host, start());
        open_table(&mut tab, &mut host, "events");
        tab.press("Hand over to an agent");
        tab.type_into("question", "Q?");
        press(&mut tab, &mut host, "Send");
        assert!(shows(&tab, words), "{code}: {}", text(&tab));
    }
}

#[test]
fn closing_the_form_keeps_the_question_and_a_new_table_closes_it() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "events");
    tab.press("Hand over to an agent");
    assert_eq!(
        buttons(&tab, "Hand over to an agent"),
        [false],
        "the form is open"
    );
    tab.type_into("question", "Which?");
    tab.press("Cancel");
    assert!(!tab.kit().hand_over().open);
    assert_eq!(buttons(&tab, "Hand over to an agent"), [true]);
    tab.press("Hand over to an agent");
    open_table(&mut tab, &mut host, "users");
    assert!(!tab.kit().hand_over().open, "another table, another form");
    assert_eq!(tab.kit().hand_over().question, "Which?");
}

#[test]
fn the_form_says_when_no_agent_can_be_reached_or_listed() {
    let mut host = Host::new("single");
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    tab.press("Hand over to an agent");
    assert!(shows(&tab, "No Claude Code agent can receive text now."));
    tab.type_into("question", "Q?");
    assert_eq!(buttons(&tab, "Send"), [false], "nobody to send to");

    let mut host = Host::new("single").deny("panes.list");
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    tab.press("Hand over to an agent");
    assert!(shows(&tab, "may not list the agents"));
    assert!(!shows(&tab, "No Claude Code agent"));
}

#[test]
fn the_agents_are_read_again_when_the_panes_change() {
    let mut host = Host::new("single");
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    tab.press("Hand over to an agent");
    assert!(shows(&tab, "No Claude Code agent"));
    host.panes = agents();
    tab.event("panes.changed", json!({}));
    assert_eq!(tab.methods(), ["panes.list"]);
    host.serve(&mut tab);
    assert!(!shows(&tab, "No Claude Code agent"));
    assert_eq!(chosen(&tab, "Agent").as_deref(), Some("Jack"));
}

// ---------------------------------------------------------------- Designed states.

#[test]
fn designed_states_no_database() {
    let (tab, _) = started("empty");
    assert_eq!(tab.kit().screen(), Screen::Ready);
    assert!(shows(&tab, "No database found"));
    let mut host = Host::new("empty");
    let french = open(&mut host, start().locale("fr"));
    assert!(shows(&french, "Aucune base de données trouvée"));
}

#[test]
fn designed_states_denied() {
    let mut host = Host::new("single").deny("command:list");
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().screen(), Screen::Denied);
    assert!(shows(&tab, "may not read the database"));
    assert_eq!(buttons(&tab, "Try again"), [true]);
    let mut host = Host::new("single").deny("command:list");
    let french = open(&mut host, start().locale("fr"));
    assert!(shows(&french, "Réglages › Kits"));

    let mut host = Host::new("single").deny("command:tables");
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().screen(), Screen::Denied);

    let mut host = Host::new("single").deny("command:rows");
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    assert_eq!(tab.kit().screen(), Screen::Denied);
}

#[test]
fn designed_states_denied_then_allowed() {
    let mut host = Host::new("single").deny("command:list");
    let mut tab = open(&mut host, start());
    assert_eq!(tab.kit().screen(), Screen::Denied);
    let mut allowed = Host::new("single").with_agents();
    press(&mut tab, &mut allowed, "Try again");
    assert_eq!(tab.kit().screen(), Screen::Ready);
    assert_eq!(tab.kit().path(), Some("app.db"));
}

#[test]
fn designed_states_no_sqlite() {
    let mut host = Host::new("nosqlite");
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().screen(), Screen::NoSqlite);
    assert!(shows(&tab, "No SQLite reader on this computer"));
    let mut host = Host::new("nosqlite");
    let french = open(&mut host, start().locale("fr"));
    assert!(shows(&french, "Aucun lecteur SQLite"));
}

#[test]
fn designed_states_locked_then_readable_once_the_writer_lets_go() {
    let mut host = Host::new("locked");
    let mut tab = open(&mut host, start());
    assert_eq!(tab.kit().database().unwrap().phase, Phase::Locked);
    assert!(shows(&tab, "The database is locked"));
    host.move_to("single");
    press(&mut tab, &mut host, "Try again");
    assert_eq!(tab.kit().database().unwrap().phase, Phase::Ready);
    assert!(shows(&tab, "events"));
}

#[test]
fn designed_states_not_a_database_gone_and_a_command_that_failed() {
    let mut host = Host::new("broken");
    let tab = open(&mut host, start().param("path", "z.sqlite"));
    assert!(
        tab.kit().files().iter().any(|file| file.path == "z.sqlite"),
        "an agent may ask for a file the listing left out"
    );
    assert_eq!(tab.kit().database().unwrap().phase, Phase::NotDatabase);
    assert!(shows(&tab, "not a SQLite database"));

    let mut host = Host::new("broken");
    let tab = open(&mut host, start().param("path", "missing.db"));
    assert_eq!(tab.kit().database().unwrap().phase, Phase::Gone);
    assert!(shows(&tab, "not in the workspace any more"));

    let mut host =
        Host::new("single").answering("tables", |_| Ok(printed(1, "", "boom: it broke\nmore")));
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().database().unwrap().phase, Phase::Failed);
    assert!(shows(&tab, "boom: it broke"));
    assert!(!shows(&tab, "more"));

    let mut host = Host::new("single").answering("list", |_| {
        Ok(json!({ "exitCode": null, "stdout": "", "stderr": "", "timedOut": true, "truncated": false }))
    });
    let tab = open(&mut host, start());
    assert_eq!(tab.kit().screen(), Screen::Failed);
    assert!(shows(&tab, "took longer than the time allowed"));
    assert_eq!(buttons(&tab, "Try again"), [true]);
}

#[test]
fn every_way_a_command_can_fail_has_its_words() {
    type Refusal = std::rc::Rc<dyn Fn() -> Result<Value, KitError>>;
    let refusals: Vec<(&str, Refusal, &str)> = vec![
        (
            "no Node.js",
            std::rc::Rc::new(|| Err(KitError::new(KitError::NOT_FOUND, "no node"))),
            "Node.js was not found on this computer",
        ),
        (
            "too many commands",
            std::rc::Rc::new(|| Err(KitError::new(KitError::BUSY, "busy"))),
            "already running the most commands it allows",
        ),
        (
            "not an answer",
            std::rc::Rc::new(|| Ok(printed(0, "this is not json", ""))),
            "not an answer",
        ),
        (
            "a crash with nothing to say",
            std::rc::Rc::new(|| {
                Ok(
                    json!({ "exitCode": 137, "stdout": "", "stderr": "", "timedOut": false, "truncated": false }),
                )
            }),
            "exit code 137",
        ),
        (
            "an error of the kit",
            std::rc::Rc::new(|| Ok(printed(0, r#"{"ok":false,"error":"bad-table"}"#, ""))),
            "no such table any more",
        ),
        (
            "an error the kit does not know",
            std::rc::Rc::new(|| {
                Ok(printed(
                    0,
                    r#"{"ok":false,"error":"made-up","detail":"why"}"#,
                    "",
                ))
            }),
            "The script could not read it: why",
        ),
    ];
    for (name, refusal, words) in refusals {
        let at_listing = refusal.clone();
        let mut host = Host::new("single").answering("list", move |_| at_listing());
        let tab = open(&mut host, start());
        assert_eq!(tab.kit().screen(), Screen::Failed, "{name}");
        assert!(shows(&tab, words), "{name}: {}", text(&tab));
        assert_eq!(buttons(&tab, "Try again"), [true], "{name}");

        let at_tables = refusal.clone();
        let mut host = Host::new("single").answering("tables", move |_| at_tables());
        let tab = open(&mut host, start());
        assert_eq!(tab.kit().database().unwrap().phase, Phase::Failed, "{name}");
        assert!(shows(&tab, words), "{name}: {}", text(&tab));
    }
}

#[test]
fn a_command_that_cannot_run_is_not_a_database_that_is_gone() {
    // Node.js missing is told as such at every step, and does not read as another failure.
    let mut host = Host::new("single").answering("rows", |_| {
        Err(KitError::new(KitError::NOT_FOUND, "no node"))
    });
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    assert_eq!(tab.kit().detail().unwrap().phase, DetailPhase::Failed);
    assert!(
        shows(&tab, "Node.js was not found on this computer"),
        "{}",
        text(&tab)
    );
    assert!(!shows(&tab, "The script could not read it"));
}

#[test]
fn a_failed_page_can_be_tried_again_and_keeps_its_structure() {
    let failing = std::rc::Rc::new(std::cell::Cell::new(true));
    let gate = failing.clone();
    let mut host = Host::new("single").answering("rows", move |args| {
        if gate.get() && args["table"] == "events" {
            Ok(printed(1, "", "disk I/O error"))
        } else {
            Ok(recorded_for(args))
        }
    });
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    assert_eq!(tab.kit().detail().unwrap().phase, DetailPhase::Failed);
    assert!(shows(&tab, "disk I/O error"));
    failing.set(false);
    press(&mut tab, &mut host, "Try again");
    assert_eq!(tab.kit().table(), Some("events"));
    assert_eq!(tab.kit().detail().unwrap().phase, DetailPhase::Ready);
    assert!(grid(&tab).is_some());
}

/// The recorded answer to a `rows` request, as `command.run`'s.
fn recorded_for(args: &std::collections::BTreeMap<String, String>) -> Value {
    let key = key("rows", args);
    let all: Value = serde_json::from_str(include_str!("fixtures/answers.json")).unwrap();
    let found = &all["worlds"]["single"][key.as_str()];
    printed(
        0,
        found["stdout"].as_str().unwrap_or_else(|| panic!("{key}")),
        "",
    )
}

#[test]
fn a_stale_answer_never_replaces_a_newer_one() {
    let mut host = Host::new("stale").with_agents();
    let mut tab = open(&mut host, start());
    assert_eq!(tab.kit().path(), Some("app.db"));
    tab.choose("Database", "b.db");
    tab.choose("Database", "app.db");
    let tables: Vec<_> = tab
        .calls()
        .iter()
        .filter(|call| call.method == "command.run" && call.params["name"] == "tables")
        .cloned()
        .collect();
    assert_eq!(tables.len(), 2);
    let (for_b, for_app) = (&tables[0], &tables[1]);
    assert_eq!(for_b.params["args"]["path"], "b.db");
    assert_eq!(for_app.params["args"]["path"], "app.db");

    // The newer answer comes first, the older one after it, and is not drawn.
    let newer = host.answer(for_app);
    tab.reply(for_app.id, newer);
    let older = Ok(printed(
        0,
        r#"{"ok":true,"size":1,"tables":[{"name":"stale_table","type":"table","rows":1,"columns":1,"viewable":true}]}"#,
        "",
    ));
    tab.reply(for_b.id, older);
    host.serve(&mut tab);
    assert_eq!(tab.kit().path(), Some("app.db"));
    assert_eq!(tab.kit().database().unwrap().phase, Phase::Ready);
    assert!(shows(&tab, "events"));
    assert!(!shows(&tab, "stale_table"));
}

#[test]
fn a_stale_page_never_replaces_a_newer_one() {
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "events");
    // Two presses on a header before any answer.
    press_header(&mut tab, 1);
    press_header(&mut tab, 1);
    let asked: Vec<_> = tab
        .calls()
        .iter()
        .filter(|call| call.method == "command.run")
        .cloned()
        .collect();
    assert_eq!(asked.len(), 2);
    assert_eq!(asked[0].params["args"]["direction"], "asc");
    assert_eq!(asked[1].params["args"]["direction"], "desc");
    let newer = host.answer(&asked[1]);
    tab.reply(asked[1].id, newer);
    let older = host.answer(&asked[0]);
    tab.reply(asked[0].id, older);
    let page = tab.kit().detail().unwrap().page.clone().unwrap();
    assert_eq!(
        page.direction,
        Direction::Desc,
        "the page of the last press stays"
    );
    assert_eq!(tab.kit().sorted(), (2, Direction::Desc));
}

#[test]
fn a_page_in_flight_does_not_cover_the_database_the_person_moved_to() {
    let mut host = Host::new("stale").with_agents();
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "events");
    press_header(&mut tab, 1);
    let rows = tab
        .calls()
        .iter()
        .find(|call| call.method == "command.run")
        .cloned()
        .unwrap();
    tab.choose("Database", "b.db");
    let late = host.answer(&rows);
    tab.reply(rows.id, late);
    host.serve(&mut tab);
    assert_eq!(tab.kit().path(), Some("b.db"));
    assert_eq!(tab.kit().table(), None);
    assert!(tab.kit().detail().is_none());
    assert!(grid(&tab).is_none());
}

// ---------------------------------------------------------------- The person's language.

#[test]
fn the_title_and_the_words_follow_hangars_language() {
    let mut host = Host::new("single").with_agents();
    let mut tab = open(&mut host, start().locale("fr"));
    assert_eq!(host.title.as_deref(), Some("Base de données"));
    assert!(shows(&tab, "Lecture seule"));
    tab.event("theme", json!({ "scheme": "dark", "locale": "en" }));
    host.serve(&mut tab);
    assert_eq!(host.title.as_deref(), Some("Database"));
    assert!(shows(&tab, "Read-only"));
    assert!(!shows(&tab, "Lecture seule"));

    // A change of scheme alone says nothing new.
    let calls = host.log.len();
    tab.event("theme", json!({ "scheme": "light", "locale": "en" }));
    assert!(tab.calls().is_empty());
    assert_eq!(host.log.len(), calls);
}

// ---------------------------------------------------------------- What a view may weigh.

#[test]
fn a_very_wide_table_is_cut_at_forty_columns_and_stays_far_under_a_message() {
    let columns: Vec<Value> = (0..200)
        .map(|i| json!({ "name": format!("column_{i}"), "type": "TEXT", "primaryKey": false }))
        .collect();
    let long: String = "x".repeat(200);
    let row: Vec<Value> = (0..200)
        .map(|_| json!({ "type": "text", "value": long, "length": 5000, "cut": true }))
        .collect();
    let document = json!({
        "ok": true, "table": "wide", "columns": columns, "rows": vec![row; 50],
        "offset": 0, "total": 500, "sort": 0, "direction": "asc", "capped": false
    })
    .to_string();
    let mut host =
        Host::new("single").answering("rows", move |_| Ok(printed(0, document.clone(), "")));
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "users");
    let drawn = grid(&tab).unwrap();
    assert_eq!(drawn.columns.len(), 40);
    assert!(drawn.rows.iter().all(|row| row.len() == 40));
    assert_eq!(drawn.rows.len(), 50);
    assert!(shows(&tab, "Only the first 40 columns are shown."));
    let weight = serde_json::to_string(tab.view()).unwrap().len();
    assert!(
        weight < 2 * 1024 * 1024,
        "{weight} bytes: a message may be 4 MB"
    );
}

#[test]
fn a_capped_page_says_so() {
    let document = json!({
        "ok": true, "table": "t", "columns": [{ "name": "a", "type": "", "primaryKey": false }],
        "rows": [[{ "type": "integer", "value": "1" }]], "offset": 0, "total": 90, "sort": 0,
        "direction": "asc", "capped": true
    })
    .to_string();
    let mut host =
        Host::new("single").answering("rows", move |_| Ok(printed(0, document.clone(), "")));
    let mut tab = open(&mut host, start());
    open_table(&mut tab, &mut host, "users");
    assert!(shows(
        &tab,
        "The page was shortened to keep the answer small."
    ));
    assert!(shows(&tab, "1–1 of 90"));
}

// ---------------------------------------------------------------- Permissions.

#[test]
fn the_tab_asks_only_for_what_its_manifest_declares() {
    // The pretend Hangar holds every call to the manifest and fails the test on one that breaks
    // it (a permission not declared, a command argument off its pattern); this is a session that
    // uses every call there is.
    let (mut tab, mut host) = started("single");
    open_table(&mut tab, &mut host, "events");
    press(&mut tab, &mut host, "Next");
    sort(&mut tab, &mut host, 1);
    open_table(&mut tab, &mut host, "users");
    press_row(&mut tab, 0);
    press(&mut tab, &mut host, SHOW_ALL);
    tab.press("Hand over to an agent");
    tab.type_into("question", "Q?");
    press(&mut tab, &mut host, "Send");
    tab.event(
        "tab.params",
        json!({ "params": { "path": "app.db" }, "caller": null }),
    );
    host.serve(&mut tab);
    let commands: std::collections::BTreeSet<String> = host
        .calls_to("command.run")
        .iter()
        .map(|params| params["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        commands.into_iter().collect::<Vec<_>>(),
        ["list", "rows", "schema", "tables"],
        "all four declared commands, and no other"
    );
    assert_eq!(
        host.data.keys().collect::<Vec<_>>(),
        ["state.json"],
        "the last pick is all the tab keeps"
    );
    assert!(
        host.calls_to("files.read").is_empty(),
        "files.read stays declared, and unused"
    );
}
