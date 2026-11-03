//! Shared by the tests: a pretend Hangar that answers the tab's calls the way Hangar does, and
//! ways to read the page a person would read.
//!
//! The pretend Hangar holds the tab to the manifest as the real one does: a call needs a
//! permission the tab declares, `command.run` names a declared command whose arguments are all
//! there and each held whole to its pattern, a text sent to an agent is at most 20 000
//! characters. A tab that breaks one of those fails the test, rather than being answered `denied`
//! (a refusal a test asks for is `Host::deny`). The commands are answered with what the real
//! script printed over the Node tests' fixture database (`fixtures/answers.json`, recorded by
//! `record.js`), or with what a test gives.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Value, json};

use hangar_kit_sdk::api::KitError;
use hangar_kit_sdk::testing::{self, Harness, StartExt};
use hangar_kit_sdk::ui::abi::Call;
use hangar_kit_sdk::ui::{Element, EventValue, Sort};
use kit_db_tab::Db;

pub const MANIFEST: &str = include_str!("../../../hangar-kit.json");
const ANSWERS: &str = include_str!("../fixtures/answers.json");

/// Strings planted in the fixture database so that a test can look for them where they must never
/// be: the same as the Node tests' (`test/helpers.js`).
pub const SECRETS: [&str; 3] = [
    "tok_live_9f3a7c51d2e84b60",
    "ada.lovelace@example.invalid",
    "pbkdf2$310000$b7e1a4c9",
];

pub fn manifest() -> &'static Value {
    static MANIFEST_JSON: OnceLock<Value> = OnceLock::new();
    MANIFEST_JSON.get_or_init(|| serde_json::from_str(MANIFEST).expect("hangar-kit.json reads"))
}

/// The permissions the tab declares.
pub fn permissions() -> Vec<String> {
    manifest()["tabs"][0]["permissions"]
        .as_array()
        .expect("the tab declares permissions")
        .iter()
        .map(|permission| permission.as_str().unwrap().to_owned())
        .collect()
}

fn answers() -> &'static Value {
    static RECORDED: OnceLock<Value> = OnceLock::new();
    RECORDED.get_or_init(|| serde_json::from_str(ANSWERS).expect("answers.json reads"))
}

/// The key `record.js` files an answer under: the command, then its arguments by name.
pub fn key(name: &str, args: &BTreeMap<String, String>) -> String {
    let mut key = name.to_owned();
    for (arg, value) in args {
        key.push_str(&format!(" {arg}={value}"));
    }
    key
}

/// A start the way Hangar gives it: the tab's declared permissions all granted.
pub fn start() -> hangar_kit_sdk::Start {
    testing::start().grant(permissions())
}

/// What a command prints: `command.run`'s answer.
pub fn printed(exit_code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Value {
    json!({ "exitCode": exit_code, "stdout": stdout.into(), "stderr": stderr.into(), "timedOut": false, "truncated": false })
}

type Override = Box<dyn Fn(&BTreeMap<String, String>) -> Result<Value, KitError>>;

pub struct Host {
    world: String,
    /// The kit's data folder.
    pub data: BTreeMap<String, String>,
    /// What `panes.list` answers.
    pub panes: Value,
    /// A method refused (`panes.send`), or a command (`command:rows`).
    denied: BTreeSet<String>,
    send_error: Option<String>,
    overrides: BTreeMap<String, Override>,
    /// Every call answered: its method and params.
    pub log: Vec<(String, Value)>,
    /// Every `panes.send`: the pane and the text.
    pub sent: Vec<(String, String)>,
    /// The latest `tab.setTitle`.
    pub title: Option<String>,
}

pub fn agents() -> Value {
    json!([
        { "name": "Jack", "agent": "claude", "role": "orchestrator", "state": "idle", "canSend": true },
        { "name": "Jude", "agent": "claude", "role": "worker", "state": "idle", "canSend": true },
        { "name": "Shell", "agent": "shell", "role": "worker", "state": "idle", "canSend": false }
    ])
}

impl Host {
    /// A workspace: `single`, `pair`, `stale`, `odd`, `empty`, `broken`, `nosqlite` or `locked`
    /// (see `record.js`).
    pub fn new(world: &str) -> Self {
        assert!(
            answers()["worlds"].get(world).is_some(),
            "no world {world:?} is recorded"
        );
        Host {
            world: world.to_owned(),
            data: BTreeMap::new(),
            panes: json!([]),
            denied: BTreeSet::new(),
            send_error: None,
            overrides: BTreeMap::new(),
            log: Vec::new(),
            sent: Vec::new(),
            title: None,
        }
    }

    pub fn with_agents(mut self) -> Self {
        self.panes = agents();
        self
    }

    pub fn remembering(mut self, path: &str) -> Self {
        self.data
            .insert("state.json".into(), json!({ "path": path }).to_string());
        self
    }

    /// Refuses a method (`panes.list`) or a command (`command:rows`), as Hangar does when the
    /// person took the permission back.
    pub fn deny(mut self, what: &str) -> Self {
        self.denied.insert(what.to_owned());
        self
    }

    /// Answers `panes.send` with this error code.
    pub fn failing_sends(mut self, code: &str) -> Self {
        self.send_error = Some(code.to_owned());
        self
    }

    /// Answers a command with this, whatever the workspace holds.
    pub fn answering(
        mut self,
        command: &str,
        answer: impl Fn(&BTreeMap<String, String>) -> Result<Value, KitError> + 'static,
    ) -> Self {
        self.overrides.insert(command.to_owned(), Box::new(answer));
        self
    }

    /// Moves to another workspace: what the person did outside the tab.
    pub fn move_to(&mut self, world: &str) {
        assert!(
            answers()["worlds"].get(world).is_some(),
            "no world {world:?}"
        );
        self.world = world.to_owned();
    }

    pub fn calls_to(&self, method: &str) -> Vec<&Value> {
        self.log
            .iter()
            .filter(|(called, _)| called == method)
            .map(|(_, params)| params)
            .collect()
    }

    /// The arguments of the commands run, `name` only.
    pub fn commands(&self, name: &str) -> Vec<BTreeMap<String, String>> {
        self.calls_to("command.run")
            .into_iter()
            .filter(|params| params["name"] == name)
            .map(args_of)
            .collect()
    }

    /// Answers every call the tab waits on, and those its answers make, until none waits.
    pub fn serve(&mut self, tab: &mut Harness<Db>) {
        for _ in 0..200 {
            let calls: Vec<Call> = tab.calls().to_vec();
            if calls.is_empty() {
                return;
            }
            for call in calls {
                let answer = self.answer(&call);
                tab.reply(call.id, answer);
            }
        }
        panic!("the tab keeps calling: {:?}", tab.methods());
    }

    /// The answer Hangar gives a call. Panics when the tab asks what it did not declare.
    pub fn answer(&mut self, call: &Call) -> Result<Value, KitError> {
        self.log.push((call.method.clone(), call.params.clone()));
        self.hold(call);
        if self.denied.contains(&call.method)
            || (call.method == "command.run"
                && self.denied.contains(&format!(
                    "command:{}",
                    call.params["name"].as_str().unwrap_or("")
                )))
        {
            return Err(KitError::new(
                KitError::DENIED,
                format!("this tab was not granted what {} needs", call.method),
            ));
        }
        let params = &call.params;
        let text_of = |name: &str| params[name].as_str().unwrap_or_default().to_owned();
        match call.method.as_str() {
            "tab.setTitle" => {
                self.title = Some(text_of("title"));
                Ok(json!({}))
            }
            "tab.setBadge" => Ok(json!({})),
            "panes.list" => Ok(self.panes.clone()),
            "panes.send" => {
                if let Some(code) = &self.send_error {
                    return Err(KitError::new(code.clone(), format!("could not: {code}")));
                }
                self.sent.push((text_of("pane"), text_of("text")));
                Ok(json!({ "queued": true, "pane": text_of("pane") }))
            }
            "data.read" => match self.data.get(&text_of("path")) {
                Some(text) => Ok(json!({ "text": text })),
                None => Err(KitError::new(KitError::NOT_FOUND, "no such file")),
            },
            "data.write" => {
                self.data.insert(text_of("path"), text_of("text"));
                Ok(json!({}))
            }
            "command.run" => self.run(&text_of("name"), &args_of(params)),
            other => panic!("the tab called {other}, which the test does not answer"),
        }
    }

    /// What Hangar's gate checks before the app sees a call: the permission, the shape.
    fn hold(&self, call: &Call) {
        let held = permissions();
        let needs = match call.method.as_str() {
            "command.run" => Some(format!(
                "command:{}",
                call.params["name"].as_str().unwrap_or("")
            )),
            "panes.list" => Some("panes.read".to_owned()),
            "panes.send" => Some("panes.send".to_owned()),
            "data.read" => Some("data.read".to_owned()),
            "data.write" => Some("data.write".to_owned()),
            "tab.setTitle" | "tab.setBadge" | "ui.toast" | "ui.confirm" | "ui.copy" => None,
            other => panic!("{other} is not a method the tab declares a permission for"),
        };
        if let Some(needs) = needs {
            assert!(
                held.contains(&needs),
                "{} needs {needs}, which the tab does not declare",
                call.method
            );
        }
        match call.method.as_str() {
            "panes.send" => {
                let text = call.params["text"].as_str().expect("a text");
                assert!(
                    text.chars().count() <= 20_000,
                    "panes.send is 20 000 characters at most"
                );
            }
            "data.write" => {
                let path = call.params["path"].as_str().expect("a path");
                assert!(
                    !path.starts_with('.') && !path.contains(".."),
                    "{path} is not a path of the data folder"
                );
                assert!(call.params["text"].as_str().expect("a text").len() <= 1 << 20);
            }
            "tab.setTitle" => {
                let title = call.params["title"].as_str().expect("a title");
                assert!(title.chars().count() <= 40 && !title.contains('\n'));
            }
            _ => {}
        }
    }

    /// `command.run`: the manifest's command, its arguments held to its patterns whole.
    fn run(&self, name: &str, args: &BTreeMap<String, String>) -> Result<Value, KitError> {
        let declared = manifest()["commands"]
            .get(name)
            .unwrap_or_else(|| panic!("{name} is not a declared command"));
        let rules = declared["args"].as_object().cloned().unwrap_or_default();
        let placeholders: BTreeSet<String> = declared["run"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item.as_str())
            .filter_map(|item| item.strip_prefix('{')?.strip_suffix('}'))
            .filter(|item| *item != "kitDir")
            .map(str::to_owned)
            .collect();
        let given: BTreeSet<String> = args.keys().cloned().collect();
        assert_eq!(
            given, placeholders,
            "{name}: every placeholder has its value, and no other"
        );
        for (arg, value) in args {
            let pattern = rules[arg]["pattern"].as_str().expect("a pattern");
            let whole = Regex::new(&format!("^(?:{pattern})$")).expect("a pattern reads");
            assert!(
                whole.is_match(value) && !value.starts_with('-'),
                "{name}: {arg}={value:?} is not what the manifest allows"
            );
        }
        if let Some(answer) = self.overrides.get(name) {
            return answer(args);
        }
        let found = &answers()["worlds"][self.world.as_str()][key(name, args)];
        assert!(
            found.is_object(),
            "no answer recorded in {:?} for `{}`: add it to native/tests/record.js and run it",
            self.world,
            key(name, args)
        );
        Ok(printed(
            found["exitCode"].as_i64().unwrap() as i32,
            found["stdout"].as_str().unwrap(),
            found["stderr"].as_str().unwrap(),
        ))
    }
}

fn args_of(params: &Value) -> BTreeMap<String, String> {
    params["args"]
        .as_object()
        .map(|args| {
            args.iter()
                .map(|(arg, value)| (arg.clone(), value.as_str().unwrap_or_default().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// The tab, started and served until it waits for nothing.
pub fn open(host: &mut Host, start: hangar_kit_sdk::Start) -> Harness<Db> {
    let mut tab = Harness::<Db>::new(start);
    host.serve(&mut tab);
    tab
}

/// A started tab with every permission, on the world, English.
pub fn started(world: &str) -> (Harness<Db>, Host) {
    let mut host = Host::new(world).with_agents();
    let tab = open(&mut host, start());
    (tab, host)
}

// ---------------------------------------------------------------- Reading the page.

/// Everything a person could read on the page, in order: titles, texts, labels, the cells of a
/// table, what a tooltip says and a select offers.
pub fn texts(tab: &Harness<Db>) -> Vec<String> {
    let mut out = tab.texts();
    tab.view().root.for_each(&mut |element| match element {
        Element::Tooltip(tooltip) => out.push(tooltip.text.clone()),
        // A text area nobody types into is a text to read.
        Element::TextArea(area) if area.on_input.is_none() => out.push(area.value.clone()),
        Element::Select(select) => {
            out.extend(select.options.iter().map(|option| option.label.clone()))
        }
        _ => {}
    });
    out
}

pub fn text(tab: &Harness<Db>) -> String {
    texts(tab).join("\n")
}

/// Whether the page shows `needle`, anywhere.
pub fn shows(tab: &Harness<Db>, needle: &str) -> bool {
    texts(tab).iter().any(|text| text.contains(needle))
}

/// The buttons labelled `label`: whether each can be pressed.
pub fn buttons(tab: &Harness<Db>, label: &str) -> Vec<bool> {
    let mut found = Vec::new();
    tab.view().root.for_each(&mut |element| {
        if let Element::Button(button) = element
            && (button.label.as_deref() == Some(label) || button.tooltip.as_deref() == Some(label))
        {
            found.push(button.on_press.is_some());
        }
    });
    found
}

/// The table of rows: its columns, and each row's cells as text.
pub struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub sort: Option<Sort>,
    pub selected: Option<u32>,
}

pub fn grid(tab: &Harness<Db>) -> Option<Grid> {
    let mut found = None;
    tab.view().root.for_each(&mut |element| {
        if let Element::Table(table) = element
            && table.id == "rows"
        {
            found = Some(Grid {
                columns: table.columns.iter().map(|c| c.title.clone()).collect(),
                rows: table
                    .rows
                    .iter()
                    .map(|row| {
                        row.cells
                            .iter()
                            .map(|cell| {
                                let mut shown = String::new();
                                cell.content.for_each(&mut |inner| {
                                    if let Element::Text(text) = inner {
                                        shown.push_str(&text.text);
                                    }
                                });
                                shown
                            })
                            .collect()
                    })
                    .collect(),
                sort: table.sort,
                selected: table.selected,
            });
        }
    });
    found
}

/// A press on a row of the table, as Hangar sends it.
pub fn press_row(tab: &mut Harness<Db>, index: u32) {
    let mut handler = None;
    tab.view().root.for_each(&mut |element| {
        if let Element::Table(table) = element
            && table.id == "rows"
        {
            handler = table.on_row;
        }
    });
    tab.ui(
        handler.expect("the table takes a press on a row"),
        Some(EventValue::Index(index)),
    );
}

/// A press on the header of column `column` (from 0), as Hangar draws it: the same column turns
/// round, another starts ascending.
pub fn press_header(tab: &mut Harness<Db>, column: u32) {
    let mut found = None;
    tab.view().root.for_each(&mut |element| {
        if let Element::Table(table) = element
            && table.id == "rows"
        {
            found = Some((table.on_sort, table.sort));
        }
    });
    let (handler, current) = found.expect("there is a table of rows");
    let sort = Sort::next(current, column);
    tab.ui(
        handler.expect("the table sorts"),
        Some(EventValue::Sort(sort)),
    );
}

/// The options of the select labelled `label`: `(value, label, disabled)`.
pub fn options(tab: &Harness<Db>, label: &str) -> Vec<(String, String, bool)> {
    let mut found = Vec::new();
    tab.view().root.for_each(&mut |element| {
        if let Element::Select(select) = element
            && select.label.as_deref() == Some(label)
        {
            found = select
                .options
                .iter()
                .map(|o| (o.value.clone(), o.label.clone(), o.disabled))
                .collect();
        }
    });
    found
}

/// What the select labelled `label` has chosen.
pub fn chosen(tab: &Harness<Db>, label: &str) -> Option<String> {
    let mut found = None;
    tab.view().root.for_each(&mut |element| {
        if let Element::Select(select) = element
            && select.label.as_deref() == Some(label)
        {
            found = select.selected.clone();
        }
    });
    found
}

/// Presses the row of the list of tables named `name`.
pub fn pick_table(tab: &mut Harness<Db>, name: &str) {
    tab.press(name);
}

/// A table is opened, and the answers served.
pub fn open_table(tab: &mut Harness<Db>, host: &mut Host, name: &str) {
    tab.press(name);
    host.serve(tab);
}

/// Sorts by header `column` (from 0) once more, and serves the answers.
pub fn sort(tab: &mut Harness<Db>, host: &mut Host, column: u32) {
    press_header(tab, column);
    host.serve(tab);
}

/// A press on a button, and the answers served.
pub fn press(tab: &mut Harness<Db>, host: &mut Host, label: &str) {
    tab.press(label);
    host.serve(tab);
}
