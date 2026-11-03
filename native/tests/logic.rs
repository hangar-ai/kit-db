//! `logic.js` ported: what the script printed is data and may be anything; nothing is drawn that
//! is not what it should be. No Hangar here, and no page.

use serde_json::json;

use kit_db_tab::logic::*;

#[test]
fn the_patterns_are_the_manifests() {
    for good in [
        "app.db",
        "data/deep/copy.sqlite3",
        "my data/app-1.db",
        ".hidden",
        "_x",
    ] {
        assert!(is_path(good), "{good}");
    }
    assert!(is_path(&format!("a{}", "b".repeat(254))));
    assert!(!is_path(&format!("a{}", "b".repeat(255))));
    for bad in [
        "",
        "-rf",
        "/etc/passwd",
        "bad ;name.db",
        "é.db",
        "a\nb",
        "a\\b",
        " lead",
    ] {
        assert!(!is_path(bad), "{bad:?}");
    }
    for good in ["users", "_t9", &"a".repeat(64)] {
        assert!(is_table(good), "{good}");
    }
    for bad in ["", "9t", "odd name", "a-b", "é", &"a".repeat(65)] {
        assert!(!is_table(bad), "{bad:?}");
    }
}

#[test]
fn the_patterns_are_the_ones_the_manifest_declares() {
    // The patterns the tab applies are held to the manifest's: a change of one without the other
    // shows here.
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../hangar-kit.json")).unwrap();
    let rows = &manifest["commands"]["rows"]["args"];
    assert_eq!(
        rows["path"]["pattern"],
        "[A-Za-z0-9_.][A-Za-z0-9_./ -]{0,254}"
    );
    assert_eq!(rows["table"]["pattern"], "[A-Za-z_][A-Za-z0-9_]{0,63}");
    assert_eq!(
        manifest["tabs"][0]["params"]["path"]["pattern"],
        rows["path"]["pattern"]
    );
}

#[test]
fn the_language_follows_the_locale() {
    for french in ["fr", "fr-FR", "FR", "fr-CA"] {
        assert_eq!(pick_lang(french), "fr", "{french}");
    }
    for english in ["en", "en-GB", "fra", "fr_CA", "", "de"] {
        assert_eq!(pick_lang(english), "en", "{english}");
    }
}

#[test]
fn an_answer_that_is_not_the_scripts_is_refused_not_drawn() {
    assert_eq!(parse_answer("not json"), None);
    assert_eq!(parse_answer("[1,2]"), None);
    assert_eq!(parse_answer(r#"{"files":[]}"#), None);
    assert_eq!(parse_answer(r#"{"ok":"yes"}"#), None);
    assert_eq!(parse_answer(""), None);
    assert_eq!(
        parse_answer(r#"{"ok":false,"error":"made-up"}"#),
        Some(Err(Failure::new("failed", "")))
    );
    assert_eq!(
        parse_answer(r#"{"ok":false,"error":7}"#),
        Some(Err(Failure::new("failed", "")))
    );
    assert_eq!(
        parse_answer(r#"{"ok":false,"error":"locked","detail":"database is locked"}"#),
        Some(Err(Failure::new("locked", "database is locked")))
    );
    for known in [
        "bad-args",
        "not-found",
        "outside",
        "not-database",
        "bad-table",
        "locked",
        "no-sqlite",
        "failed",
    ] {
        let printed = json!({ "ok": false, "error": known }).to_string();
        assert_eq!(
            parse_answer(&printed),
            Some(Err(Failure::new(known, ""))),
            "{known}"
        );
    }
    let long = json!({ "ok": false, "error": "failed", "detail": "x".repeat(900) }).to_string();
    let Some(Err(failure)) = parse_answer(&long) else {
        panic!("a failure");
    };
    assert_eq!(failure.detail.chars().count(), 300, "a detail is cut");
    let with_bom = format!("\u{feff}{}", json!({ "ok": true, "files": [] }));
    assert!(
        matches!(parse_answer(&with_bom), Some(Ok(_))),
        "a byte order mark is no harm"
    );
}

#[test]
fn what_a_command_came_to_is_told_apart() {
    let ok = r#"{"ok":true,"files":[]}"#;
    assert!(classify_run(Some(0), false, ok, "").is_ok());
    assert_eq!(
        classify_run(None, true, "", ""),
        Err(Failure::new("timeout", ""))
    );
    assert_eq!(
        classify_run(Some(0), false, "oops", ""),
        Err(Failure::new("unreadable", ""))
    );
    assert_eq!(
        classify_run(Some(1), false, ok, "\n\nboom: it broke\nmore\n"),
        Err(Failure::new("failed", "boom: it broke")),
        "the first line of stderr, and what was printed is not trusted on a failure"
    );
    assert_eq!(
        classify_run(Some(2), false, "", ""),
        Err(Failure::new("failed", "exit code 2"))
    );
    assert_eq!(
        classify_run(None, false, "", ""),
        Err(Failure::new("failed", "no exit code"))
    );
    assert_eq!(classify_error("denied", "no"), Failure::new("denied", ""));
    assert_eq!(
        classify_error("not_found", "no node"),
        Failure::new("not_found", "")
    );
    assert_eq!(
        classify_error("busy", "16 waiting"),
        Failure::new("busy", "")
    );
    assert_eq!(
        classify_error("rate_limited", &"m".repeat(500))
            .detail
            .chars()
            .count(),
        200
    );
    assert_eq!(classify_error("bad_args", "path").error, "failed");
}

#[test]
fn the_files_are_checked() {
    let files = clean_files(&json!({ "files": [
        { "path": "app.db", "size": 20480, "modified": 1700000000000u64, "usable": true },
        { "path": "bad ;name.db", "size": 1, "modified": 2, "usable": true },
        { "path": "no.db", "usable": false },
        { "path": 7 },
        "app.db",
        null,
    ] }));
    let usable: Vec<(&str, bool)> = files.iter().map(|f| (f.path.as_str(), f.usable)).collect();
    assert_eq!(
        usable,
        [("app.db", true), ("bad ;name.db", false), ("no.db", false)]
    );
    assert_eq!(
        (files[0].size, files[0].modified),
        (20480, 1_700_000_000_000)
    );
    assert_eq!(clean_files(&json!({})), []);
    assert_eq!(clean_files(&json!({ "files": "app.db" })), []);
    let many: Vec<_> = (0..150)
        .map(|i| json!({ "path": format!("f{i}.db"), "usable": true }))
        .collect();
    assert_eq!(clean_files(&json!({ "files": many })).len(), 100);
}

#[test]
fn the_tables_are_checked() {
    let tables = clean_tables(&json!({
        "size": 4096.9,
        "tables": [
            { "name": "a b", "rows": -3, "viewable": true },
            { "rows": 1 },
            null,
            { "name": "users", "type": "view", "rows": "7", "columns": 3.8, "viewable": true },
            { "name": "events", "type": "table", "rows": 120, "columns": 3, "viewable": "yes" },
        ]
    }));
    assert_eq!(tables.size, 4096);
    assert_eq!(tables.tables.len(), 3);
    let odd = &tables.tables[0];
    assert!(
        !odd.viewable,
        "a name outside the pattern is listed but never opened"
    );
    assert_eq!(odd.rows, 0, "a count below zero is none");
    let users = &tables.tables[1];
    assert_eq!(
        (users.kind, users.rows, users.columns, users.viewable),
        (Kind::View, 0, 3, true)
    );
    assert!(!tables.tables[2].viewable, "viewable is true, not truthy");
    let many: Vec<_> = (0..600)
        .map(|i| json!({ "name": format!("t{i}"), "viewable": true }))
        .collect();
    assert_eq!(clean_tables(&json!({ "tables": many })).tables.len(), 500);
}

#[test]
fn a_structure_is_checked() {
    let schema = clean_schema(&json!({
        "table": "users", "type": "table",
        "columns": [
            { "name": "id", "type": "INTEGER", "notNull": false, "primaryKey": true, "default": null },
            { "name": "token", "type": "TEXT", "notNull": true, "default": "'none'" },
            "nonsense",
            { "name": 7 },
        ],
        "columnsTruncated": true,
        "indexes": [{ "name": "users_email", "unique": true }, { "unique": true }, { "name": "plain" }]
    }));
    assert_eq!(schema.table, "users");
    assert_eq!(
        schema.columns.len(),
        3,
        "a column with a bad name is still a column, named nothing"
    );
    assert!(schema.columns[0].primary_key && !schema.columns[0].not_null);
    assert_eq!(schema.columns[0].default, None);
    assert_eq!(schema.columns[1].default.as_deref(), Some("'none'"));
    assert!(schema.columns[1].not_null);
    assert_eq!(schema.columns[2].name, "");
    assert!(schema.columns_truncated);
    let indexes: Vec<(&str, bool)> = schema
        .indexes
        .iter()
        .map(|i| (i.name.as_str(), i.unique))
        .collect();
    assert_eq!(indexes, [("users_email", true), ("plain", false)]);
}

#[test]
fn a_page_of_rows_is_checked() {
    let page = clean_rows(&json!({
        "columns": [{ "name": "a", "type": "INTEGER", "primaryKey": true }, "junk"],
        "rows": [
            [
                { "type": "integer", "value": "1" },
                { "type": "text", "value": "x".repeat(150_000), "length": 150_000, "cut": true },
                { "type": "blob", "length": 8 },
                { "type": "real", "value": "3.5" },
                { "type": "evil", "value": "x" },
                { "type": "text" },
                7,
                null,
            ],
            "not a row",
        ],
        "offset": 50, "total": 120, "sort": 2, "direction": "desc", "capped": true
    }));
    assert_eq!(page.columns.len(), 1);
    assert!(page.columns[0].primary_key);
    assert_eq!(page.rows.len(), 1);
    let row = &page.rows[0];
    assert_eq!(row[0], Cell::Integer("1".into()));
    let Cell::Text { value, length, cut } = &row[1] else {
        panic!("a text");
    };
    assert_eq!(
        (value.chars().count(), *length, *cut),
        (100_000, 150_000, true)
    );
    assert_eq!(row[2], Cell::Blob { length: 8 });
    assert_eq!(row[3], Cell::Real("3.5".into()));
    assert_eq!(
        row[4],
        Cell::Null,
        "a type the kit does not know is nothing"
    );
    assert_eq!(
        row[5],
        Cell::Text {
            value: String::new(),
            length: 0,
            cut: false
        }
    );
    assert_eq!((row[6].clone(), row[7].clone()), (Cell::Null, Cell::Null));
    assert_eq!((page.offset, page.total, page.sort), (50, 120, 2));
    assert_eq!(page.direction, Direction::Desc);
    assert!(page.capped);
    let defaults = clean_rows(&json!({ "direction": "sideways" }));
    assert_eq!(
        (defaults.direction, defaults.rows.len(), defaults.capped),
        (Direction::Asc, 0, false)
    );
    assert_eq!(
        clean_rows(&json!({ "rows": [[{ "type": "evil", "value": "x" }]] })).rows[0][0],
        Cell::Null
    );
}

#[test]
fn a_number_is_kept_as_the_script_wrote_it() {
    let page = clean_rows(&json!({ "rows": [[
        { "type": "integer", "value": "9007199254740993" },
        { "type": "real", "value": "0.30000000000000004" },
    ]] }));
    assert_eq!(page.rows[0][0], Cell::Integer("9007199254740993".into()));
    assert_eq!(page.rows[0][1], Cell::Real("0.30000000000000004".into()));
}

#[test]
fn a_pick_is_remembered_and_read_back_if_it_is_still_a_path() {
    assert_eq!(remember("app.db"), r#"{"path":"app.db"}"#);
    assert_eq!(
        remembered_path(&remember("data/my app.db")).as_deref(),
        Some("data/my app.db")
    );
    for bad in [
        "",
        "not json",
        "[]",
        r#"{"path":7}"#,
        r#"{"path":"/etc/x"}"#,
        r#"{}"#,
        "null",
    ] {
        assert_eq!(remembered_path(bad), None, "{bad}");
    }
}

#[test]
fn sorting_goes_ascending_then_descending_then_not_at_all() {
    use Direction::{Asc, Desc};
    assert_eq!(next_sort(0, Asc, 3), (3, Asc));
    assert_eq!(next_sort(3, Asc, 3), (3, Desc));
    assert_eq!(next_sort(3, Desc, 3), (0, Asc));
    assert_eq!(
        next_sort(3, Desc, 1),
        (1, Asc),
        "another column starts again"
    );
    assert_eq!(next_sort(3, Asc, 1), (1, Asc));
}

#[test]
fn numbers_are_told_by_their_declared_type() {
    for numeric in [
        "INTEGER",
        "int",
        "BIGINT",
        "REAL",
        "NUMERIC(10,2)",
        "FLOAT",
        "DOUBLE PRECISION",
        "DECIMAL",
    ] {
        assert!(is_numeric_type(numeric), "{numeric}");
    }
    for other in ["", "TEXT", "BLOB", "VARCHAR(20)", "DATE"] {
        assert!(!is_numeric_type(other), "{other}");
    }
}

#[test]
fn white_space_is_one_space() {
    assert_eq!(one_line("a\n\tb  c\r\n"), "a b c");
    assert_eq!(one_line("\n  x"), "x");
    assert_eq!(one_line(""), "");
    assert_eq!(one_line("tab\there"), "tab here");
}

#[test]
fn an_agent_is_the_orchestrator_if_there_is_one_else_the_first() {
    let agent = |name: &str, orchestrator| Agent {
        name: name.into(),
        orchestrator,
    };
    assert_eq!(default_target(&[]), None);
    assert_eq!(
        default_target(&[agent("Jude", false), agent("Ana", false)]),
        Some("Jude")
    );
    assert_eq!(
        default_target(&[agent("Jude", false), agent("Jack", true)]),
        Some("Jack")
    );
}

fn column(name: &str, ty: &str, primary_key: bool) -> Column {
    Column {
        name: name.into(),
        ty: ty.into(),
        not_null: false,
        primary_key,
        default: None,
    }
}

#[test]
fn the_brief_names_the_table_and_the_question_and_nothing_else() {
    let columns = [
        column("id", "INTEGER", true),
        column("name", "", false),
        column("bio", "TEXT", false),
    ];
    let brief = brief_for(&Brief {
        path: "data/app.db",
        table: "users",
        kind: Kind::Table,
        total: 3,
        columns: &columns,
        question: "Why?\nBecause.",
    });
    let lines: Vec<&str> = brief.lines().collect();
    assert!(lines[0].starts_with(
        "The person is looking at a database of this project in Hangar's Database kit"
    ));
    assert!(lines[0].contains("No row of the table is in it"));
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], "Database file: data/app.db");
    assert_eq!(lines[3], "Table: users (3 rows)");
    assert_eq!(
        lines[4],
        "Columns: id INTEGER (primary key), name, bio TEXT"
    );
    assert_eq!(lines[6], "Question: Why?");
    assert_eq!(
        lines[7], "Because.",
        "the question is the person's own words, whole"
    );
    assert!(brief.ends_with("unless the person asks you to."));
}

#[test]
fn the_brief_counts_one_row_and_says_when_it_is_a_view_or_has_no_column() {
    let brief = brief_for(&Brief {
        path: "a.db",
        table: "recent",
        kind: Kind::View,
        total: 1,
        columns: &[],
        question: "Q",
    });
    assert!(brief.contains("View: recent (1 row)"), "{brief}");
    assert!(brief.contains("Columns: none"));
}
