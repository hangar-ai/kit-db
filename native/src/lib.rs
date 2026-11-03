//! The Database kit's tab, native: Rust built to WebAssembly, drawn by Hangar itself.
//!
//! It opens a SQLite file of the workspace read-only, through the kit's four declared commands
//! (`list`, `tables`, `schema`, `rows`, all `node tools/db.js` with a fixed verb): the tables with
//! their counts, a table's structure, its rows sorted by a click on a column and read page by
//! page. Values may be secrets. They stay on this screen: nothing goes to an agent unless the
//! person clicks "Hand over to an agent", and then only the table's name, its columns and their
//! question, never a row. No cell value is ever logged, toasted or sent.
//!
//! The files, as the HTML tab's (`tabs/db/`, which Hangar 1.x still opens):
//!
//! - `logic.rs`: `logic.js`, the script's documents checked, the patterns, the brief.
//! - `strings.rs`: the English and French words.
//! - this file: `controller.js`, the state, its messages and what each one does.
//! - `view.rs`: `view.js`, the page as a tree of widgets.
//!
//! A command's answer, a call's refusal, a stale reply: every one is a state of the page, drawn.
//! A reply that comes after the person moved on (another database, another page) carries the
//! token of the request it answers, and is dropped if the token is no longer the latest.

pub mod logic;
pub mod strings;
mod view;

use std::collections::BTreeMap;

use hangar_kit_sdk::api::{DataText, Queued, TabParamsAnswer, Theme};
use hangar_kit_sdk::prelude::*;
use hangar_kit_sdk::ui::Sort;

use logic::{
    Agent, Brief, CELL_CHARS, Direction, FULL_CHARS, Failure, Outcome, PAGE_SIZE, Page, Schema,
    TableInfo,
};

/// What the whole page is, before a database is looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The databases are being listed.
    Loading,
    /// They were listed.
    Ready,
    /// A command is not allowed.
    Denied,
    /// Nothing on the computer can read SQLite.
    NoSqlite,
    /// The listing failed.
    Failed,
}

/// Where the opening of a database is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loading,
    Ready,
    Locked,
    NotDatabase,
    /// The file is not in the workspace any more.
    Gone,
    Failed,
}

/// The database being looked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Database {
    pub phase: Phase,
    pub error: Option<Failure>,
    pub size: u64,
    pub tables: Vec<TableInfo>,
}

impl Database {
    fn loading() -> Self {
        Database {
            phase: Phase::Loading,
            error: None,
            size: 0,
            tables: Vec::new(),
        }
    }

    fn stopped(phase: Phase, error: Failure) -> Self {
        Database {
            phase,
            error: Some(error),
            size: 0,
            tables: Vec::new(),
        }
    }
}

/// The table being looked at: its structure and its page of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    pub phase: DetailPhase,
    pub error: Option<Failure>,
    pub schema: Option<Schema>,
    pub page: Option<Page>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailPhase {
    Loading,
    Ready,
    Failed,
}

/// Which of the table's two views is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Rows,
    Structure,
}

/// A text value read whole, or on its way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Full {
    Loading,
    Done { value: String, cut: bool },
    Failed,
}

/// Why a hand-over was not sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// `denied`, `not_found`, `rate_limited`, `unavailable` or `failed`.
    pub code: String,
    pub detail: String,
    pub agent: String,
}

/// The form that hands a table and a question to an agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandOver {
    pub open: bool,
    pub question: String,
    pub sending: bool,
    /// The agent it was handed to.
    pub sent: Option<String>,
    pub problem: Option<Problem>,
}

/// The first reading: the tab starts when the databases are listed, the last pick is read and the
/// agents are known, so that it opens the right one at once.
struct Boot {
    /// A database an agent asked for, in the tab's params.
    asked: Option<String>,
    remembered: Option<String>,
    remembered_in: bool,
    panes_in: bool,
    listed: Option<Outcome>,
}

/// The two answers a table's detail waits for: its structure (only when it is new) and its rows.
struct Load {
    token: u32,
    with_schema: bool,
    previous_schema: Option<Schema>,
    schema: Option<Outcome>,
    rows: Option<Outcome>,
}

pub struct Db {
    t: hangar_kit_sdk::i18n::Strings,
    /// The language the tab's title was last set in.
    lang: &'static str,
    boot: Option<Boot>,
    screen: Screen,
    /// Why the listing failed.
    error: Option<Failure>,
    files: Vec<logic::DbFile>,
    /// The database being looked at.
    path: Option<String>,
    db: Option<Database>,
    /// The table being looked at.
    table: Option<String>,
    detail: Option<Detail>,
    /// The column the rows are sorted by, from 1; 0 for none.
    sort: u32,
    direction: Direction,
    /// The first row of the page.
    offset: u32,
    panel: Panel,
    /// The row pressed in the table, by its index in the page.
    row: Option<usize>,
    /// `(row, column)` of a cut text → its whole value.
    full: BTreeMap<(usize, usize), Full>,
    agents: Vec<Agent>,
    panes_denied: bool,
    /// The agent the person picked.
    choice: Option<String>,
    hand_over: HandOver,
    // A newer request makes an older answer obsolete.
    db_token: u32,
    detail_token: u32,
    refresh_token: u32,
    load: Option<Load>,
}

#[derive(Clone)]
pub enum Message {
    // The person.
    Refresh,
    Pick(String),
    SelectTable(String),
    Panel(String),
    Sort(Sort),
    Row(usize),
    GoTo(u32),
    ToggleFull(usize, usize),
    Choose(String),
    OpenHandOver,
    CloseHandOver,
    Question(String),
    SendHandOver,
    // The answers.
    Remembered(Result<DataText, KitError>),
    Panes(Result<Vec<api::Pane>, KitError>),
    Booted(Outcome),
    Refreshed {
        token: u32,
        keep: Option<String>,
        table: Option<String>,
        outcome: Outcome,
    },
    Tables {
        token: u32,
        reselect: Option<String>,
        outcome: Outcome,
    },
    Schema(u32, Outcome),
    Rows(u32, Outcome),
    Whole {
        token: u32,
        row: usize,
        column: usize,
        outcome: Outcome,
    },
    Sent(String, Result<Queued, KitError>),
    // Hangar.
    PanesChanged,
    Asked(TabParamsAnswer),
    Theme(Theme),
}

/// A command's answer, or Hangar's refusal of it, as the document or the failure it comes to.
fn classify(answer: Result<api::CommandAnswer, KitError>) -> Outcome {
    match answer {
        Ok(done) => logic::classify_run(done.exit_code, done.timed_out, &done.stdout, &done.stderr),
        Err(error) => Err(logic::classify_error(&error.code, &error.message)),
    }
}

/// Runs one of the declared commands: `name` and its arguments, all strings.
fn run(
    cx: &mut Context<Message>,
    name: &str,
    args: &[(&str, String)],
    then: impl FnOnce(Outcome) -> Message + 'static,
) {
    let args = args
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect();
    cx.call(
        api::CommandRun {
            name: name.to_owned(),
            args,
        },
        move |answer| then(classify(answer)),
    );
}

impl Kit for Db {
    type Message = Message;

    fn init(start: &Start, cx: &mut Context<Message>) -> Self {
        let asked = start
            .tab
            .params
            .get("path")
            .filter(|path| logic::is_path(path))
            .cloned();
        let db = Db {
            t: strings::strings(&start.theme.locale),
            lang: logic::pick_lang(&start.theme.locale),
            boot: Some(Boot {
                asked,
                remembered: None,
                remembered_in: false,
                panes_in: false,
                listed: None,
            }),
            screen: Screen::Loading,
            error: None,
            files: Vec::new(),
            path: None,
            db: None,
            table: None,
            detail: None,
            sort: 0,
            direction: Direction::Asc,
            offset: 0,
            panel: Panel::Rows,
            row: None,
            full: BTreeMap::new(),
            agents: Vec::new(),
            panes_denied: false,
            choice: None,
            hand_over: HandOver::default(),
            db_token: 0,
            detail_token: 0,
            refresh_token: 0,
            load: None,
        };
        db.set_title(cx);
        cx.call(
            api::DataRead {
                path: logic::STATE_FILE.to_owned(),
            },
            Message::Remembered,
        );
        cx.call(api::PanesList {}, Message::Panes);
        run(cx, "list", &[], Message::Booted);
        db
    }

    fn update(&mut self, message: Message, cx: &mut Context<Message>) {
        match message {
            Message::Refresh => self.refresh(cx),
            Message::Pick(path) => {
                if logic::is_path(&path) && self.path.as_deref() != Some(path.as_str()) {
                    self.open_database(Some(path), true, None, cx);
                }
            }
            Message::SelectTable(name) => self.select_table(&name, cx),
            Message::Panel(id) => {
                self.panel = if id == "structure" {
                    Panel::Structure
                } else {
                    Panel::Rows
                }
            }
            Message::Sort(sort) => self.sort_by(sort.column + 1, cx),
            Message::Row(index) => {
                self.row = if self.row == Some(index) {
                    None
                } else {
                    Some(index)
                }
            }
            Message::GoTo(offset) => self.go_to(offset, cx),
            Message::ToggleFull(row, column) => self.toggle_full(row, column, cx),
            Message::Choose(agent) => self.choice = Some(agent),
            Message::OpenHandOver => {
                self.hand_over.open = true;
                self.hand_over.sent = None;
                self.hand_over.problem = None;
            }
            Message::CloseHandOver => {
                self.hand_over.open = false;
                self.hand_over.problem = None;
            }
            Message::Question(question) => self.hand_over.question = question,
            Message::SendHandOver => self.send_hand_over(cx),

            Message::Remembered(answer) => {
                if let Some(boot) = &mut self.boot {
                    boot.remembered = answer
                        .ok()
                        .and_then(|read| logic::remembered_path(&read.text));
                    boot.remembered_in = true;
                    self.boot_step(cx);
                }
            }
            Message::Panes(answer) => self.panes_in(answer, cx),
            Message::Booted(outcome) => {
                if let Some(boot) = &mut self.boot {
                    boot.listed = Some(outcome);
                    self.boot_step(cx);
                }
            }
            Message::Refreshed {
                token,
                keep,
                table,
                outcome,
            } => {
                if token == self.refresh_token {
                    self.refreshed(keep, table, outcome, cx);
                }
            }
            Message::Tables {
                token,
                reselect,
                outcome,
            } => {
                if token == self.db_token {
                    self.tables_in(reselect, outcome, cx);
                }
            }
            Message::Schema(token, outcome) => {
                if token == self.detail_token
                    && let Some(load) = &mut self.load
                {
                    load.schema = Some(outcome);
                    self.load_step();
                }
            }
            Message::Rows(token, outcome) => {
                if token == self.detail_token
                    && let Some(load) = &mut self.load
                {
                    load.rows = Some(outcome);
                    self.load_step();
                }
            }
            Message::Whole {
                token,
                row,
                column,
                outcome,
            } => {
                if token == self.detail_token && self.full.contains_key(&(row, column)) {
                    let cell = outcome
                        .ok()
                        .and_then(|document| logic::clean_rows(&document).rows.into_iter().next())
                        .and_then(|cells| cells.into_iter().nth(column));
                    let whole = match cell {
                        Some(logic::Cell::Text { value, cut, .. }) => Full::Done { value, cut },
                        _ => Full::Failed,
                    };
                    self.full.insert((row, column), whole);
                }
            }
            Message::Sent(agent, answer) => self.sent(agent, answer, cx),

            Message::PanesChanged => cx.call(api::PanesList {}, Message::Panes),
            Message::Asked(asked) => {
                let path = asked
                    .params
                    .get("path")
                    .filter(|path| logic::is_path(path))
                    .cloned();
                if let Some(path) = path {
                    match &mut self.boot {
                        Some(boot) => boot.asked = Some(path),
                        None => self.open_asked(path, cx),
                    }
                }
            }
            Message::Theme(theme) => {
                self.t.set_locale(theme.locale);
                let lang = logic::pick_lang(self.t.locale());
                if lang != self.lang {
                    self.lang = lang;
                    self.set_title(cx);
                }
            }
        }
    }

    fn view(&self) -> Element<Message> {
        self.page()
    }

    fn subscriptions(&self) -> Subscriptions<Message> {
        Subscriptions::none()
            .panes_changed(Message::PanesChanged)
            .tab_params(Message::Asked)
            .theme(Message::Theme)
    }
}

impl Db {
    /// The tab's name in the tab strip, in the person's language.
    fn set_title(&self, cx: &mut Context<Message>) {
        cx.call_quietly(api::TabSetTitle {
            title: self.t.t("tab"),
        });
    }

    /// A failure that changes the whole page: the commands are not allowed, or nothing can read
    /// SQLite. Whether it was one.
    fn fatal(&mut self, failure: &Failure) -> bool {
        match failure.error.as_str() {
            "denied" => self.screen = Screen::Denied,
            "no-sqlite" => self.screen = Screen::NoSqlite,
            _ => return false,
        }
        true
    }

    /// The databases could not be listed: the page says why, or what to allow.
    fn listing_failed(&mut self, failure: Failure) {
        if !self.fatal(&failure) {
            self.screen = Screen::Failed;
            self.error = Some(failure);
        }
    }

    fn panes_in(&mut self, answer: Result<Vec<api::Pane>, KitError>, cx: &mut Context<Message>) {
        match answer {
            Ok(panes) => {
                self.agents = panes
                    .into_iter()
                    .filter(|pane| pane.agent == "claude" && pane.can_send)
                    .map(|pane| Agent {
                        orchestrator: pane.role.as_deref() == Some("orchestrator"),
                        name: pane.name,
                    })
                    .collect();
                self.panes_denied = false;
            }
            Err(error) => {
                self.agents.clear();
                self.panes_denied = error.is(KitError::DENIED);
            }
        }
        if let Some(boot) = &mut self.boot {
            boot.panes_in = true;
            self.boot_step(cx);
        }
    }

    /// Once the listing, the last pick and the agents are all in: opens the database an agent
    /// asked for, else the last pick if it is still there, else the first that can be opened.
    fn boot_step(&mut self, cx: &mut Context<Message>) {
        let ready = self
            .boot
            .as_ref()
            .is_some_and(|boot| boot.remembered_in && boot.panes_in && boot.listed.is_some());
        if !ready {
            return;
        }
        let Some(boot) = self.boot.take() else {
            return;
        };
        match boot.listed {
            Some(Ok(document)) => {
                self.screen = Screen::Ready;
                self.error = None;
                self.files = logic::clean_files(&document);
                let mut path = boot.asked.clone().or(boot.remembered);
                if let Some(chosen) = &path
                    && !self.files.iter().any(|file| &file.path == chosen)
                {
                    if boot.asked.is_some() {
                        self.files.insert(0, logic::DbFile::asked(chosen));
                    } else {
                        // What was remembered is gone.
                        path = None;
                    }
                }
                let path = path.or_else(|| self.first_usable());
                self.open_database(path, false, None, cx);
            }
            Some(Err(failure)) => self.listing_failed(failure),
            None => {}
        }
    }

    fn first_usable(&self) -> Option<String> {
        self.files
            .iter()
            .find(|file| file.usable)
            .map(|file| file.path.clone())
    }

    /// Lists the databases again and reads the one open again, the table too if it is still there.
    fn refresh(&mut self, cx: &mut Context<Message>) {
        self.refresh_token += 1;
        let (token, keep, table) = (self.refresh_token, self.path.clone(), self.table.clone());
        if self.screen != Screen::Ready {
            self.screen = Screen::Loading;
        }
        run(cx, "list", &[], move |outcome| Message::Refreshed {
            token,
            keep,
            table,
            outcome,
        });
    }

    fn refreshed(
        &mut self,
        keep: Option<String>,
        table: Option<String>,
        outcome: Outcome,
        cx: &mut Context<Message>,
    ) {
        match outcome {
            Ok(document) => {
                self.screen = Screen::Ready;
                self.error = None;
                self.files = logic::clean_files(&document);
            }
            Err(failure) => self.listing_failed(failure),
        }
        if self.screen != Screen::Ready {
            return;
        }
        if let Some(kept) = &keep
            && !self.files.iter().any(|file| &file.path == kept)
        {
            self.files.insert(0, logic::DbFile::asked(kept));
        }
        let path = keep.or_else(|| self.first_usable());
        self.open_database(path, false, table, cx);
    }

    /// An agent asked for a database: it joins the list if it is not there, and opens.
    fn open_asked(&mut self, path: String, cx: &mut Context<Message>) {
        if !self.files.iter().any(|file| file.path == path) {
            self.files.insert(0, logic::DbFile::asked(&path));
        }
        self.open_database(Some(path), true, None, cx);
    }

    /// Forgets the table: what a new database starts from.
    fn reset_table(&mut self) {
        self.table = None;
        self.detail = None;
        self.load = None;
        self.sort = 0;
        self.direction = Direction::Asc;
        self.offset = 0;
        self.row = None;
        self.full.clear();
        self.hand_over.open = false;
        self.hand_over.sent = None;
        self.hand_over.problem = None;
    }

    /// Opens `path` (none: nothing can be), remembering it when the person chose it. `reselect`
    /// is the table to look at again once its tables are in, a refresh's.
    fn open_database(
        &mut self,
        path: Option<String>,
        save: bool,
        reselect: Option<String>,
        cx: &mut Context<Message>,
    ) {
        self.db_token += 1;
        self.detail_token += 1;
        let token = self.db_token;
        self.reset_table();
        self.db = path.as_ref().map(|_| Database::loading());
        self.path = path.clone();
        let Some(path) = path else {
            return;
        };
        if save {
            cx.call_quietly(api::DataWrite {
                path: logic::STATE_FILE.to_owned(),
                text: logic::remember(&path),
            });
        }
        run(cx, "tables", &[("path", path)], move |outcome| {
            Message::Tables {
                token,
                reselect,
                outcome,
            }
        });
    }

    fn tables_in(&mut self, reselect: Option<String>, outcome: Outcome, cx: &mut Context<Message>) {
        match outcome {
            Ok(document) => {
                let clean = logic::clean_tables(&document);
                // The only table there is, if it can be opened, is opened.
                let only = match clean.tables.as_slice() {
                    [only] if only.viewable => Some(only.name.clone()),
                    _ => None,
                };
                let again = reselect.filter(|name| {
                    clean
                        .tables
                        .iter()
                        .any(|table| &table.name == name && table.viewable)
                });
                self.db = Some(Database {
                    phase: Phase::Ready,
                    error: None,
                    size: clean.size,
                    tables: clean.tables,
                });
                if let Some(name) = again.or(only) {
                    self.select_table(&name, cx);
                }
            }
            Err(failure) => {
                if self.fatal(&failure) {
                    self.db = None;
                } else {
                    let phase = match failure.error.as_str() {
                        "locked" => Phase::Locked,
                        "not-database" => Phase::NotDatabase,
                        "not-found" => Phase::Gone,
                        _ => Phase::Failed,
                    };
                    self.db = Some(Database::stopped(phase, failure));
                }
            }
        }
    }

    fn select_table(&mut self, name: &str, cx: &mut Context<Message>) {
        let viewable = self
            .db
            .as_ref()
            .is_some_and(|db| db.tables.iter().any(|t| t.name == name && t.viewable));
        if !viewable {
            return;
        }
        self.table = Some(name.to_owned());
        self.sort = 0;
        self.direction = Direction::Asc;
        self.offset = 0;
        self.row = None;
        self.full.clear();
        self.hand_over.open = false;
        self.hand_over.sent = None;
        self.hand_over.problem = None;
        self.load_detail(true, cx);
    }

    /// Asks for the table's rows as the sort and the page say, and for its structure when it is
    /// new. The page and the structure already shown stay in sight while the rows come.
    fn load_detail(&mut self, with_schema: bool, cx: &mut Context<Message>) {
        let (Some(path), Some(table)) = (self.path.clone(), self.table.clone()) else {
            return;
        };
        self.detail_token += 1;
        let token = self.detail_token;
        let previous = self.detail.take();
        let previous_schema = previous.as_ref().and_then(|detail| detail.schema.clone());
        self.detail = Some(Detail {
            phase: DetailPhase::Loading,
            error: None,
            schema: if with_schema {
                None
            } else {
                previous_schema.clone()
            },
            page: if with_schema {
                None
            } else {
                previous.and_then(|detail| detail.page)
            },
        });
        self.load = Some(Load {
            token,
            with_schema,
            previous_schema,
            schema: None,
            rows: None,
        });
        if with_schema {
            run(
                cx,
                "schema",
                &[("path", path.clone()), ("table", table.clone())],
                move |outcome| Message::Schema(token, outcome),
            );
        }
        run(
            cx,
            "rows",
            &[
                ("path", path),
                ("table", table),
                ("sort", self.sort.to_string()),
                ("direction", self.direction.arg().to_owned()),
                ("offset", self.offset.to_string()),
                ("limit", PAGE_SIZE.to_string()),
                ("chars", CELL_CHARS.to_string()),
            ],
            move |outcome| Message::Rows(token, outcome),
        );
    }

    /// Both answers in: the detail, or why there is none.
    fn load_step(&mut self) {
        let done = self.load.as_ref().is_some_and(|load| {
            load.rows.is_some() && (!load.with_schema || load.schema.is_some())
        });
        if !done {
            return;
        }
        let Some(load) = self.load.take() else {
            return;
        };
        debug_assert_eq!(load.token, self.detail_token);
        let (schema, rows) = (load.schema, load.rows);
        let bad = [&schema, &rows]
            .into_iter()
            .flatten()
            .find_map(|outcome| outcome.as_ref().err().cloned());
        if let Some(failure) = bad {
            if self.fatal(&failure) {
                self.db = None;
            } else {
                self.detail = Some(Detail {
                    phase: DetailPhase::Failed,
                    error: Some(failure),
                    schema: load.previous_schema,
                    page: None,
                });
            }
            return;
        }
        let schema = match schema {
            Some(Ok(document)) => Some(logic::clean_schema(&document)),
            _ => load.previous_schema,
        };
        let page = rows
            .and_then(Result::ok)
            .map(|document| logic::clean_rows(&document));
        self.detail = Some(Detail {
            phase: DetailPhase::Ready,
            error: None,
            schema,
            page,
        });
    }

    /// A press on the header of column `index`, from 1: ascending, then descending, then not at
    /// all. Hangar turns a column round on each press; the third press (a column that is
    /// descending, pressed again) is read here as the end of the sort, from the state.
    fn sort_by(&mut self, index: u32, cx: &mut Context<Message>) {
        if self.table.is_none() {
            return;
        }
        (self.sort, self.direction) = logic::next_sort(self.sort, self.direction, index);
        self.offset = 0;
        self.row = None;
        self.full.clear();
        self.load_detail(false, cx);
    }

    fn go_to(&mut self, offset: u32, cx: &mut Context<Message>) {
        if self.table.is_none() {
            return;
        }
        self.offset = offset;
        self.row = None;
        self.full.clear();
        self.load_detail(false, cx);
    }

    /// The whole value of the text at `row` and `column` of the page shown, or back to the cut one.
    fn toggle_full(&mut self, row: usize, column: usize, cx: &mut Context<Message>) {
        let key = (row, column);
        let page = self.detail.as_ref().and_then(|detail| detail.page.as_ref());
        let (Some(page), Some(path), Some(table)) = (page, self.path.clone(), self.table.clone())
        else {
            self.full.remove(&key);
            return;
        };
        if self.full.remove(&key).is_some() {
            return;
        }
        let args = [
            ("path", path),
            ("table", table),
            ("sort", page.sort.to_string()),
            ("direction", page.direction.arg().to_owned()),
            ("offset", (page.offset + row as u64).to_string()),
            ("limit", "1".to_owned()),
            ("chars", FULL_CHARS.to_string()),
        ];
        let token = self.detail_token;
        self.full.insert(key, Full::Loading);
        run(cx, "rows", &args, move |outcome| Message::Whole {
            token,
            row,
            column,
            outcome,
        });
    }

    /// The agent a hand-over goes to: the one picked, else the orchestrator, else the first.
    fn target(&self) -> Option<String> {
        self.choice
            .clone()
            .or_else(|| logic::default_target(&self.agents).map(str::to_owned))
    }

    /// Sends the table and the question to the target agent: no row is read for it.
    fn send_hand_over(&mut self, cx: &mut Context<Message>) {
        let question = self.hand_over.question.trim().to_owned();
        let meta = self.table.as_ref().and_then(|name| {
            self.db
                .as_ref()
                .and_then(|db| db.tables.iter().find(|table| &table.name == name))
        });
        let schema = self
            .detail
            .as_ref()
            .and_then(|detail| detail.schema.as_ref());
        let (Some(agent), Some(path), Some(meta), Some(schema)) =
            (self.target(), self.path.as_ref(), meta, schema)
        else {
            return;
        };
        if question.is_empty() || self.hand_over.sending {
            return;
        }
        let text = logic::brief_for(&Brief {
            path,
            table: &meta.name,
            kind: meta.kind,
            total: meta.rows,
            columns: &schema.columns,
            question: &question,
        });
        self.hand_over.sending = true;
        self.hand_over.problem = None;
        let pane = agent.clone();
        cx.call(api::PanesSend { pane, text }, move |answer| {
            Message::Sent(agent, answer)
        });
    }

    fn sent(&mut self, agent: String, answer: Result<Queued, KitError>, cx: &mut Context<Message>) {
        match answer {
            Ok(_) => {
                self.hand_over = HandOver {
                    sent: Some(agent),
                    ..HandOver::default()
                };
            }
            Err(error) => {
                let code = match error.code.as_str() {
                    "denied" | "not_found" | "rate_limited" | "unavailable" => error.code.clone(),
                    _ => "failed".to_owned(),
                };
                self.hand_over.sending = false;
                self.hand_over.problem = Some(Problem {
                    code,
                    detail: error.message.clone(),
                    agent,
                });
                // The agent is gone, or cannot be written to now: who can be?
                if error.is(KitError::NOT_FOUND) || error.is(KitError::UNAVAILABLE) {
                    cx.call(api::PanesList {}, Message::Panes);
                }
            }
        }
    }

    // ------------------------------------------------------------ What the tests read.

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn error(&self) -> Option<&Failure> {
        self.error.as_ref()
    }

    pub fn files(&self) -> &[logic::DbFile] {
        &self.files
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn database(&self) -> Option<&Database> {
        self.db.as_ref()
    }

    pub fn table(&self) -> Option<&str> {
        self.table.as_deref()
    }

    pub fn detail(&self) -> Option<&Detail> {
        self.detail.as_ref()
    }

    /// The column rows are sorted by, from 1, and the way.
    pub fn sorted(&self) -> (u32, Direction) {
        (self.sort, self.direction)
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn full(&self, row: usize, column: usize) -> Option<&Full> {
        self.full.get(&(row, column))
    }

    pub fn hand_over(&self) -> &HandOver {
        &self.hand_over
    }
}

export_kit!(Db);
