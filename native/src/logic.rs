//! The tab's pure logic, `logic.js` ported: the script's documents, checked and turned into what
//! the page shows, the rules of the manifest's patterns, the sort that cycles, and the brief an
//! agent is handed. No Hangar and no SDK type in it, so every rule is tested on its own.
//!
//! What the script printed is data and may be anything: nothing is trusted, everything is read
//! through the `text`, `count` and `list` helpers, and what is not what it should be is dropped
//! or becomes its empty value.

use serde_json::Value;

/// Where the kit keeps the person's last pick, in its data folder.
pub const STATE_FILE: &str = "state.json";
/// Rows per page.
pub const PAGE_SIZE: u32 = 50;
/// Characters of a text value the script cuts a page's cells at.
pub const CELL_CHARS: u32 = 200;
/// Characters of a text value read when the person asks for the whole one.
pub const FULL_CHARS: u32 = 100_000;

/// What the script answers with, besides the document of a verb that went well.
const ERRORS: [&str; 8] = [
    "bad-args",
    "not-found",
    "outside",
    "not-database",
    "bad-table",
    "locked",
    "no-sqlite",
    "failed",
];

// ---------------------------------------------------------------- Patterns.

/// `[A-Za-z0-9_.][A-Za-z0-9_./ -]{0,254}`: the manifest's pattern for a database's path, which
/// Hangar holds every value to, whole.
pub fn is_path(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphanumeric() || first == '_' || first == '.') {
        return false;
    }
    let rest = chars.as_str();
    rest.chars().count() <= 254
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | ' ' | '-'))
}

/// `[A-Za-z_][A-Za-z0-9_]{0,63}`: the manifest's pattern for a table's name.
pub fn is_table(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    let rest = chars.as_str();
    rest.chars().count() <= 63 && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The language a locale is drawn in: French for `fr` (and `fr-FR`), else English.
pub fn pick_lang(language: &str) -> &'static str {
    let lower = language.to_ascii_lowercase();
    match lower.strip_prefix("fr") {
        Some(rest)
            if !rest
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') =>
        {
            "fr"
        }
        _ => "en",
    }
}

// ---------------------------------------------------------------- The script's answers.

/// What went wrong, in the kit's codes: the script's own (`locked`, `no-sqlite`, `not-database`,
/// `not-found`, `outside`, `bad-table`, `bad-args`, `failed`) and those of the call, `denied`,
/// `not_found` (Node.js is not on the computer), `busy`, `timeout`, `unreadable`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub error: String,
    pub detail: String,
}

impl Failure {
    pub fn new(error: &str, detail: impl Into<String>) -> Self {
        Failure {
            error: error.to_owned(),
            detail: detail.into(),
        }
    }
}

/// A command's document, or why there is none.
pub type Outcome = Result<Value, Failure>;

/// `command.run`'s answer, turned into the script's document or a failure: a time-out, an exit
/// that is not 0, or something printed that is not an answer, are told apart.
pub fn classify_run(
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: &str,
    stderr: &str,
) -> Outcome {
    if timed_out {
        return Err(Failure::new("timeout", ""));
    }
    if exit_code == Some(0) {
        if let Some(parsed) = parse_answer(stdout) {
            return parsed;
        }
        return Err(Failure::new("unreadable", ""));
    }
    let detail = match stderr.lines().find(|line| !line.is_empty()) {
        Some(line) => line.to_owned(),
        None => match exit_code {
            Some(code) => format!("exit code {code}"),
            None => "no exit code".to_owned(),
        },
    };
    Err(Failure::new("failed", detail))
}

/// A call Hangar refused or could not do. `denied` and `busy` are told apart; `not_found` is
/// `command.run` not finding Node.js.
pub fn classify_error(code: &str, message: &str) -> Failure {
    match code {
        "denied" | "not_found" | "busy" => Failure::new(code, ""),
        _ => Failure::new("failed", text_of(message, 200)),
    }
}

/// What a script printed: one JSON document with `ok`, else none.
pub fn parse_answer(source: &str) -> Option<Outcome> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let answer: Value = serde_json::from_str(source).ok()?;
    let ok = answer.as_object()?.get("ok")?.as_bool()?;
    if ok {
        return Some(Ok(answer));
    }
    let error = answer["error"].as_str().filter(|e| ERRORS.contains(e));
    Some(Err(Failure::new(
        error.unwrap_or("failed"),
        text(&answer["detail"], 300),
    )))
}

fn is_record(value: &Value) -> bool {
    value.is_object()
}

/// A string cut to `max` characters; none for anything else.
fn text(value: &Value, max: usize) -> String {
    value.as_str().map(|s| text_of(s, max)).unwrap_or_default()
}

fn text_of(source: &str, max: usize) -> String {
    source.chars().take(max).collect()
}

/// A whole number from zero up; anything else is 0.
fn count(value: &Value) -> u64 {
    match value.as_f64() {
        Some(number) if number.is_finite() && number >= 0.0 => number.floor() as u64,
        _ => 0,
    }
}

fn list(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn flag(value: &Value) -> bool {
    value.as_bool() == Some(true)
}

// ---------------------------------------------------------------- What the page shows.

/// A database to pick from, `list`'s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbFile {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    /// Whether its path is one the commands take.
    pub usable: bool,
}

impl DbFile {
    /// A file an agent asked for that the listing left out.
    pub fn asked(path: &str) -> Self {
        DbFile {
            path: path.to_owned(),
            size: 0,
            modified: 0,
            usable: true,
        }
    }
}

/// `list`'s files, checked: the databases to pick from.
pub fn clean_files(answer: &Value) -> Vec<DbFile> {
    list(&answer["files"])
        .iter()
        .filter(|file| is_record(file) && file["path"].is_string())
        .take(100)
        .map(|file| DbFile {
            path: text(&file["path"], 255),
            size: count(&file["size"]),
            modified: count(&file["modified"]),
            usable: flag(&file["usable"]) && file["path"].as_str().is_some_and(is_path),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Table,
    View,
}

fn kind(value: &Value) -> Kind {
    if value.as_str() == Some("view") {
        Kind::View
    } else {
        Kind::Table
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableInfo {
    pub name: String,
    pub kind: Kind,
    pub rows: u64,
    pub columns: u64,
    /// Whether its name is one the commands take.
    pub viewable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tables {
    pub size: u64,
    pub tables: Vec<TableInfo>,
}

/// `tables`' answer, checked.
pub fn clean_tables(answer: &Value) -> Tables {
    Tables {
        size: count(&answer["size"]),
        tables: list(&answer["tables"])
            .iter()
            .filter(|item| is_record(item) && item["name"].is_string())
            .take(500)
            .map(|item| TableInfo {
                name: text(&item["name"], 200),
                kind: kind(&item["type"]),
                rows: count(&item["rows"]),
                columns: count(&item["columns"]),
                viewable: flag(&item["viewable"]) && item["name"].as_str().is_some_and(is_table),
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub ty: String,
    pub not_null: bool,
    pub primary_key: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub table: String,
    pub kind: Kind,
    pub columns: Vec<Column>,
    pub columns_truncated: bool,
    pub indexes: Vec<Index>,
}

/// `schema`'s answer, checked.
pub fn clean_schema(answer: &Value) -> Schema {
    Schema {
        table: text(&answer["table"], 64),
        kind: kind(&answer["type"]),
        columns: list(&answer["columns"])
            .iter()
            .filter(|column| is_record(column))
            .take(200)
            .map(|column| Column {
                name: text(&column["name"], 200),
                ty: text(&column["type"], 60),
                not_null: flag(&column["notNull"]),
                primary_key: flag(&column["primaryKey"]),
                default: column["default"].as_str().map(|d| text_of(d, 200)),
            })
            .collect(),
        columns_truncated: flag(&answer["columnsTruncated"]),
        indexes: list(&answer["indexes"])
            .iter()
            .filter(|index| is_record(index) && index["name"].is_string())
            .take(50)
            .map(|index| Index {
                name: text(&index["name"], 200),
                unique: flag(&index["unique"]),
            })
            .collect(),
    }
}

/// One cell of `rows`, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Null,
    /// Told by its size, never its content.
    Blob {
        length: u64,
    },
    Text {
        value: String,
        /// How many characters the value has in the database.
        length: u64,
        /// Whether `value` is the start of it.
        cut: bool,
    },
    /// A number as SQL writes it, so that none loses a digit.
    Integer(String),
    Real(String),
}

fn clean_cell(cell: &Value) -> Cell {
    if !is_record(cell) {
        return Cell::Null;
    }
    match cell["type"].as_str() {
        Some("blob") => Cell::Blob {
            length: count(&cell["length"]),
        },
        Some("text") => Cell::Text {
            value: text(&cell["value"], FULL_CHARS as usize),
            length: count(&cell["length"]),
            cut: flag(&cell["cut"]),
        },
        Some("integer") => Cell::Integer(text(&cell["value"], 400)),
        Some("real") => Cell::Real(text(&cell["value"], 400)),
        _ => Cell::Null,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Asc,
    Desc,
}

impl Direction {
    /// What the `rows` command takes.
    pub fn arg(self) -> &'static str {
        match self {
            Direction::Asc => "asc",
            Direction::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowColumn {
    pub name: String,
    pub ty: String,
    pub primary_key: bool,
}

/// A page of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub columns: Vec<RowColumn>,
    pub rows: Vec<Vec<Cell>>,
    pub offset: u64,
    pub total: u64,
    /// The column it is sorted by, from 1; 0 for none.
    pub sort: u64,
    pub direction: Direction,
    /// Whether the script shortened the page to keep its answer small.
    pub capped: bool,
}

/// `rows`' answer, checked.
pub fn clean_rows(answer: &Value) -> Page {
    Page {
        columns: list(&answer["columns"])
            .iter()
            .filter(|column| is_record(column))
            .take(200)
            .map(|column| RowColumn {
                name: text(&column["name"], 200),
                ty: text(&column["type"], 60),
                primary_key: flag(&column["primaryKey"]),
            })
            .collect(),
        rows: list(&answer["rows"])
            .iter()
            .filter_map(Value::as_array)
            .take(200)
            .map(|row| row.iter().take(200).map(clean_cell).collect())
            .collect(),
        offset: count(&answer["offset"]),
        total: count(&answer["total"]),
        sort: count(&answer["sort"]),
        direction: if answer["direction"].as_str() == Some("desc") {
            Direction::Desc
        } else {
            Direction::Asc
        },
        capped: flag(&answer["capped"]),
    }
}

/// The path the last visit remembered: `{"path": "app.db"}`, if it is one the commands take.
pub fn remembered_path(state: &str) -> Option<String> {
    let state: Value = serde_json::from_str(state).ok()?;
    state["path"]
        .as_str()
        .filter(|p| is_path(p))
        .map(str::to_owned)
}

/// What is written to remember a pick.
pub fn remember(path: &str) -> String {
    serde_json::json!({ "path": path }).to_string()
}

// ---------------------------------------------------------------- Sorting.

/// A press on the header of column `index` (from 1), the table being sorted by `sort` the way
/// `direction` says: the first press sorts ascending, the next descending, the next not at all.
/// Another column starts again, ascending.
pub fn next_sort(sort: u32, direction: Direction, index: u32) -> (u32, Direction) {
    if sort != index {
        (index, Direction::Asc)
    } else if direction == Direction::Asc {
        (index, Direction::Desc)
    } else {
        (0, Direction::Asc)
    }
}

/// Whether a column's declared type is a number, to align its values to the end: the page's
/// `/INT|REAL|NUM|FLOA|DOUB|DEC/i`.
pub fn is_numeric_type(ty: &str) -> bool {
    let ty = ty.to_ascii_uppercase();
    ["INT", "REAL", "NUM", "FLOA", "DOUB", "DEC"]
        .iter()
        .any(|word| ty.contains(word))
}

/// A value on one line, as a cell holds it: every run of white space is one space.
pub fn one_line(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut space = false;
    for c in value.chars() {
        if c.is_whitespace() {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------- Agents.

/// A Claude Code agent that can be sent text now: a pane whose `agent` is `claude` and whose
/// `canSend` is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub orchestrator: bool,
}

/// The agent a hand-over goes to by default: the orchestrator when there is one, else the first
/// that can be reached.
pub fn default_target(reachable: &[Agent]) -> Option<&str> {
    reachable
        .iter()
        .find(|agent| agent.orchestrator)
        .or(reachable.first())
        .map(|agent| agent.name.as_str())
}

/// What a brief says of a table.
pub struct Brief<'a> {
    pub path: &'a str,
    pub table: &'a str,
    pub kind: Kind,
    pub total: u64,
    pub columns: &'a [Column],
    pub question: &'a str,
}

/// The text sent to an agent: the database's path, the table, its columns and the person's
/// question, and nothing read from a row. English whatever the page's language: it is for an
/// agent. The question is the person's own words, so it is not put between markers.
pub fn brief_for(brief: &Brief) -> String {
    let columns: Vec<String> = brief
        .columns
        .iter()
        .map(|column| {
            let mut described = column.name.clone();
            if !column.ty.is_empty() {
                described.push(' ');
                described.push_str(&column.ty);
            }
            if column.primary_key {
                described.push_str(" (primary key)");
            }
            described
        })
        .collect();
    let columns = if columns.is_empty() {
        "none".to_owned()
    } else {
        columns.join(", ")
    };
    [
        "The person is looking at a database of this project in Hangar's Database kit and handed you a table with a question. This message comes from the kit, on the person's click. No row of the table is in it: its values may be secrets.".to_owned(),
        String::new(),
        format!("Database file: {}", brief.path),
        format!(
            "{}: {} ({} {})",
            if brief.kind == Kind::View { "View" } else { "Table" },
            brief.table,
            brief.total,
            if brief.total == 1 { "row" } else { "rows" }
        ),
        format!("Columns: {columns}"),
        String::new(),
        format!("Question: {}", brief.question),
        String::new(),
        "To look at the data yourself, use the kit's read-only script, whose commands are listed in its skill, or the project's own tools. Do not copy values you read into messages, files or commits unless the person asks you to.".to_owned(),
    ]
    .join("\n")
}
