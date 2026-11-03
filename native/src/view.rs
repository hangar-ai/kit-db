//! The page, `view.js` ported: the controller's state drawn with Hangar's widgets, and no colour
//! of its own. A value read from a row is only ever text.

use hangar_kit_sdk::prelude::*;
use hangar_kit_sdk::ui::{Sort, TableColumn};

use crate::logic::{self, Cell as DbCell, DbFile, Direction, Failure, Kind, Page, Schema};
use crate::{Database, Db, DetailPhase, Full, Message, Panel, Phase, Screen};

type Handled = hangar_kit_sdk::Handler<Message>;

/// Columns drawn, past which a note says so: a view stays far under the 4 MB a message may be.
const MAX_COLUMNS: usize = 40;
/// Characters of a value a cell holds before it is cut at the edge; the tooltip holds more.
const CELL_SHOWN: usize = 60;
/// Characters of a value its tooltip holds, which is what the script cut it at.
const TOOLTIP_SHOWN: usize = 200;
/// What a table's height may grow to before it scrolls inside itself.
const TABLE_MAX_HEIGHT: f32 = 360.0;
/// A table's header and a row, in logical pixels, to give it no more height than it needs.
const TABLE_HEADER: f32 = 30.0;
const TABLE_ROW: f32 = 28.0;
/// How tall the whole value of a cell is before its text area scrolls.
const WHOLE_HEIGHT: f32 = 200.0;

impl Db {
    fn n(&self, value: u64) -> String {
        format::number(value as f64, 0, self.t.locale())
    }

    fn retry(&self) -> Element<Message> {
        button(self.t.t("retry"))
            .small()
            .on_press(Message::Refresh)
            .into()
    }

    /// The failure a command's answer comes to, in words. A code with no words of its own reads
    /// as the script failing, with the detail it gave, or else the code.
    fn error_text(&self, failure: &Failure) -> String {
        let shown = if failure.detail.is_empty() {
            failure.error.as_str()
        } else {
            failure.detail.as_str()
        };
        let key = format!("error.{}", failure.error);
        let text = self.t.tv(&key, &[("detail", shown)]);
        if text == key {
            self.t.tv("error.failed", &[("detail", shown)])
        } else {
            text
        }
    }

    pub(crate) fn page(&self) -> Element<Message> {
        let mut children: Vec<Element<Message>> = vec![self.header().into()];
        match self.screen {
            Screen::Loading => children.push(loading(self.t.t("loading")).into()),
            Screen::Denied => children.push(
                denied(self.t.t("deniedTitle"))
                    .text(self.t.t("deniedText"))
                    .action(self.retry())
                    .into(),
            ),
            Screen::NoSqlite => children.push(
                failed(self.t.t("noSqliteTitle"))
                    .text(self.t.t("noSqliteText"))
                    .action(self.retry())
                    .into(),
            ),
            Screen::Failed => {
                let failure = self
                    .error
                    .clone()
                    .unwrap_or_else(|| Failure::new("failed", ""));
                children.push(
                    failed(self.t.t("errorTitle"))
                        .text(self.error_text(&failure))
                        .action(self.retry())
                        .into(),
                );
            }
            Screen::Ready => children.extend(self.ready()),
        }
        page(children).into()
    }

    fn header(&self) -> hangar_kit_sdk::ui::Header<Handled> {
        let mut head = header(self.t.t("tab"));
        if let Some(db) = &self.db
            && self.path.is_some()
            && db.phase == Phase::Ready
        {
            head = head
                .state(state_label(Status::Neutral, self.t.t("readOnly")))
                .state(text(format::bytes(db.size, self.t.locale())).small().dim());
        }
        if matches!(self.screen, Screen::Ready | Screen::Failed) {
            head = head.action(
                button(self.t.t("refresh"))
                    .small()
                    .on_press(Message::Refresh),
            );
        }
        head
    }

    fn ready(&self) -> Vec<Element<Message>> {
        if self.files.is_empty() {
            return vec![
                empty(self.t.t("noDbTitle"))
                    .icon(Icon::Info)
                    .text(self.t.t("noDbText"))
                    .action(
                        button(self.t.t("refresh"))
                            .small()
                            .on_press(Message::Refresh),
                    )
                    .into(),
            ];
        }
        let mut out: Vec<Element<Message>> = vec![self.picker()];
        match &self.db {
            // Files there are, and none the commands take.
            None => out.push(
                empty(self.t.t("noDbTitle"))
                    .icon(Icon::Info)
                    .text(self.t.t("unusable"))
                    .into(),
            ),
            Some(db) if db.phase == Phase::Loading => out.push(loading(self.t.t("loading")).into()),
            Some(db) if db.phase != Phase::Ready => out.push(self.database_problem(db)),
            Some(db) => {
                out.push(banner(Notice::Info, self.t.t("secretNote")).into());
                out.push(self.tables_section(db));
                if let Some(table) = &self.table
                    && self.detail.is_some()
                {
                    out.push(self.table_block(table));
                }
            }
        }
        out
    }

    /// Which database: a choice among several, the one there is otherwise.
    fn picker(&self) -> Element<Message> {
        if let [only] = self.files.as_slice() {
            let mut lines: Vec<Element<Message>> = vec![
                text(self.t.t("databaseLabel")).small().dim().into(),
                text(only.path.clone()).mono().into(),
            ];
            if !only.usable {
                lines.push(text(self.t.t("unusable")).small().dim().into());
            }
            return column(lines).spacing(2.0).into();
        }
        let options = self.files.iter().map(|file: &DbFile| {
            let label = if file.usable {
                file.path.clone()
            } else {
                format!("{} — {}", file.path, self.t.t("unusable"))
            };
            select_option(file.path.clone(), label).disabled(!file.usable)
        });
        select(options, self.path.clone())
            .label(self.t.t("databaseLabel"))
            .on_select(Message::Pick)
            .into()
    }

    fn database_problem(&self, db: &Database) -> Element<Message> {
        match db.phase {
            Phase::Locked => failed(self.t.t("lockedTitle"))
                .icon(Icon::Lock)
                .text(self.t.t("lockedText"))
                .action(self.retry())
                .into(),
            Phase::NotDatabase => failed(self.t.t("notDatabaseTitle"))
                .text(self.t.t("notDatabaseText"))
                .into(),
            Phase::Gone => failed(self.t.t("goneTitle"))
                .text(self.t.t("goneText"))
                .action(self.retry())
                .into(),
            _ => {
                let failure = db
                    .error
                    .clone()
                    .unwrap_or_else(|| Failure::new("failed", ""));
                failed(self.t.t("errorTitle"))
                    .text(self.error_text(&failure))
                    .action(self.retry())
                    .into()
            }
        }
    }

    fn tables_section(&self, db: &Database) -> Element<Message> {
        let body: Element<Message> = if db.tables.is_empty() {
            text(self.t.t("noTables")).dim().into()
        } else {
            list(db.tables.iter().map(|info| {
                let mut sub = vec![self.t.tn("columnsCount", info.columns as i64, &[])];
                if info.kind == Kind::View {
                    sub.insert(0, self.t.t("view"));
                }
                if !info.viewable {
                    sub.push(self.t.t("unsupportedName"));
                }
                let row = item(info.name.clone())
                    .sub(sub.join(" · "))
                    .end(
                        text(self.t.tn("rowsCount", info.rows as i64, &[]))
                            .numeric()
                            .small()
                            .dim(),
                    )
                    .tooltip(info.name.clone())
                    .selected(self.table.as_deref() == Some(info.name.as_str()));
                if info.viewable {
                    row.on_press(Message::SelectTable(info.name.clone()))
                } else {
                    row
                }
            }))
            .into()
        };
        section(self.t.t("tables"), [body])
            .count(self.n(db.tables.len() as u64))
            .into()
    }

    /// The table being looked at: its name, the hand-over, the two panels.
    fn table_block(&self, name: &str) -> Element<Message> {
        let detail = self.detail.as_ref();
        let can_hand = detail.is_some_and(|d| d.schema.is_some()) && !self.hand_over.open;
        let head = row![
            text(name.to_owned())
                .mono()
                .weight(Weight::Semibold)
                .truncate(),
            fill(),
            button(self.t.t("handOver"))
                .small()
                .on_press_maybe(can_hand.then_some(Message::OpenHandOver)),
        ]
        .spacing(8.0)
        .align(Align::Center);

        let mut out: Vec<Element<Message>> = vec![head.into()];
        if let Some(panel) = self.hand_over_panel() {
            out.push(panel);
        }
        let shown = match self.panel {
            Panel::Structure => "structure",
            Panel::Rows => "rows",
        };
        out.push(
            tabs(
                [
                    tab_item("rows", self.t.t("rowsTitle")),
                    tab_item("structure", self.t.t("structure")),
                ],
                shown,
            )
            .on_select(Message::Panel)
            .into(),
        );
        match (self.panel, detail) {
            (Panel::Structure, Some(d)) => match &d.schema {
                Some(schema) => out.push(self.structure_panel(schema)),
                None => out.push(loading(self.t.t("loading")).into()),
            },
            (Panel::Rows, Some(d)) => out.extend(self.rows_panel(d)),
            _ => {}
        }
        column(out).spacing(8.0).into()
    }

    fn structure_panel(&self, schema: &Schema) -> Element<Message> {
        let columns = list(schema.columns.iter().map(|column| {
            let mut bits = vec![if column.ty.is_empty() {
                "—".to_owned()
            } else {
                column.ty.clone()
            }];
            if column.primary_key {
                bits.push(self.t.t("key"));
            }
            if column.not_null {
                bits.push(self.t.t("required"));
            }
            if let Some(default) = &column.default {
                bits.push(self.t.tv("defaultValue", &[("value", default)]));
            }
            item(column.name.clone()).mono().sub(bits.join(" · "))
        }))
        .group();
        let mut out: Vec<Element<Message>> = vec![columns.into()];
        if schema.columns_truncated {
            out.push(text(self.t.t("columnsCut")).small().dim().into());
        }
        if !schema.indexes.is_empty() {
            let chips = schema.indexes.iter().map(|index| {
                chip(if index.unique {
                    format!("{} · {}", index.name, self.t.t("unique"))
                } else {
                    index.name.clone()
                })
                .into()
            });
            out.push(text(self.t.t("indexes")).small().dim().into());
            out.push(row(chips).spacing(6.0).wrap().into());
        }
        column(out).spacing(8.0).into()
    }

    fn rows_panel(&self, detail: &crate::Detail) -> Vec<Element<Message>> {
        if detail.phase == DetailPhase::Failed {
            let failure = detail
                .error
                .clone()
                .unwrap_or_else(|| Failure::new("failed", ""));
            return vec![
                failed(self.t.t("errorTitle"))
                    .text(self.error_text(&failure))
                    .action(self.retry())
                    .into(),
            ];
        }
        let Some(page) = &detail.page else {
            return vec![loading(self.t.t("loading")).into()];
        };
        if page.rows.is_empty() {
            return vec![text(self.t.t("emptyTable")).dim().into()];
        }
        let mut out: Vec<Element<Message>> = Vec::new();
        if page.capped {
            out.push(banner(Notice::Warning, self.t.t("capped")).into());
        }
        if page.columns.len() > MAX_COLUMNS {
            out.push(
                text(
                    self.t
                        .tv("columnsShown", &[("n", &self.n(MAX_COLUMNS as u64))]),
                )
                .small()
                .dim()
                .into(),
            );
        }
        out.push(self.data_table(page));
        if let Some(whole) = self.whole_values(page) {
            out.push(whole);
        }
        out.push(self.pager(page));
        out
    }

    /// The rows as a table Hangar draws; the order, a click on a header, is the script's to give.
    fn data_table(&self, page: &Page) -> Element<Message> {
        let shown = page.columns.len().min(MAX_COLUMNS);
        let columns = page
            .columns
            .iter()
            .take(shown)
            .enumerate()
            .map(|(index, column)| {
                let longest = page
                    .rows
                    .iter()
                    .filter_map(|row| row.get(index))
                    .map(|cell| cell_text(cell, self).chars().count().min(24))
                    .chain([column.name.chars().count() + 2])
                    .max()
                    .unwrap_or(0);
                let width = (longest as f32 * 7.6 + 20.0).clamp(56.0, 210.0);
                let head = TableColumn::new(column.name.clone())
                    .sortable()
                    .width(width);
                if logic::is_numeric_type(&column.ty) {
                    head.numeric()
                } else {
                    head
                }
            });
        let rows = page
            .rows
            .iter()
            .map(|row| table_row(row.iter().take(shown).map(|value| self.data_cell(value))));
        let height = (TABLE_HEADER + TABLE_ROW * page.rows.len() as f32).min(TABLE_MAX_HEIGHT);
        let mut data = table(columns, rows)
            .id("rows")
            .height(height)
            .on_sort(Message::Sort)
            .on_row(Message::Row);
        if self.sort > 0 {
            let column = self.sort - 1;
            data = data.sort(if self.direction == Direction::Desc {
                Sort::descending(column)
            } else {
                Sort::ascending(column)
            });
        }
        if let Some(row) = self.row {
            data = data.selected(row as u32);
        }
        scroll(data)
            .axis(Axis::Horizontal)
            .height(Length::Shrink)
            .into()
    }

    /// A cell: a value is text and nothing else, on one line, cut at the edge of its column.
    fn data_cell(&self, shown: &DbCell) -> hangar_kit_sdk::ui::Cell<Handled> {
        match shown {
            DbCell::Null => cell(code(self.t.t("null")).dim().truncate()),
            DbCell::Blob { length } => cell(
                code(self.t.tn("blob", *length as i64, &[]))
                    .dim()
                    .truncate(),
            ),
            DbCell::Integer(value) | DbCell::Real(value) => {
                cell(code(value.clone()).numeric().truncate())
            }
            DbCell::Text { value, cut, .. } => {
                let content = code(cell_text_of(value, *cut)).truncate();
                if value.chars().count() > CELL_SHOWN || *cut {
                    cell(tooltip(
                        content,
                        value.chars().take(TOOLTIP_SHOWN).collect::<String>(),
                    ))
                } else {
                    cell(content)
                }
            }
        }
    }

    /// Under the table, the long texts of the row pressed: each can be read whole, up to
    /// 100 000 characters, and folded again.
    fn whole_values(&self, page: &Page) -> Option<Element<Message>> {
        let row_index = self.row?;
        let row = page.rows.get(row_index)?;
        let items: Vec<_> = row
            .iter()
            .enumerate()
            .take(MAX_COLUMNS)
            .filter_map(|(column, cell)| {
                let DbCell::Text { length, cut, .. } = cell else {
                    return None;
                };
                let whole = self.full.get(&(row_index, column));
                if !*cut && whole.is_none() {
                    return None;
                }
                let name = page.columns.get(column).map(|c| c.name.clone())?;
                let n = self.n(*length);
                let press = Message::ToggleFull(row_index, column);
                let mut entry = item(name).mono().wrap();
                entry = match whole {
                    None => entry.body(
                        button(self.t.tv("showFull", &[("n", &n)]))
                            .small()
                            .on_press(press),
                    ),
                    Some(Full::Loading) => entry.body(button(self.t.t("loadingFull")).small()),
                    Some(Full::Failed) => entry
                        .body(text(self.t.t("fullFailed")).small().tone(Tone::Danger))
                        .body(
                            button(self.t.tv("showFull", &[("n", &n)]))
                                .small()
                                .on_press(press),
                        ),
                    Some(Full::Done { value, cut }) => {
                        let mut entry = entry
                            .body(button(self.t.t("showLess")).small().on_press(press))
                            .body(
                                // Read-only, so that it can be selected and copied.
                                text_area(format!("whole-{row_index}-{column}"), value.clone())
                                    .mono()
                                    .height(WHOLE_HEIGHT),
                            );
                        if *cut {
                            entry = entry.body(
                                text(
                                    self.t
                                        .tv("fullCut", &[("n", &self.n(logic::FULL_CHARS as u64))]),
                                )
                                .small()
                                .dim(),
                            );
                        }
                        entry
                    }
                };
                Some(entry)
            })
            .collect();
        if items.is_empty() {
            None
        } else {
            Some(list(items).group().into())
        }
    }

    fn pager(&self, page: &Page) -> Element<Message> {
        let from = if page.rows.is_empty() {
            0
        } else {
            page.offset + 1
        };
        let to = page.offset + page.rows.len() as u64;
        let previous = button(self.t.t("previous"))
            .small()
            .on_press_maybe((page.offset > 0).then(|| {
                Message::GoTo(page.offset.saturating_sub(logic::PAGE_SIZE as u64) as u32)
            }));
        let next = button(self.t.t("next")).small().on_press_maybe(
            (to < page.total)
                .then(|| Message::GoTo((page.offset + logic::PAGE_SIZE as u64) as u32)),
        );
        let range = self.t.tv(
            "range",
            &[
                ("from", &self.n(from)),
                ("to", &self.n(to)),
                ("total", &self.n(page.total)),
            ],
        );
        let line =
            row![previous, fill(), text(range).numeric().dim(), fill(), next].align(Align::Center);
        bar(vec![line.into()]).foot().into()
    }

    /// Handing the table to an agent: the question, who, and whether it went. Never a row.
    fn hand_over_panel(&self) -> Option<Element<Message>> {
        let hand = &self.hand_over;
        if !hand.open {
            return hand.sent.as_ref().map(|agent| {
                banner(Notice::Success, self.t.tv("sentTo", &[("agent", agent)])).into()
            });
        }
        let target = self.target();
        let mut out: Vec<Element<Message>> = vec![
            text(self.t.t("handOverTitle"))
                .weight(Weight::Semibold)
                .into(),
            text_area("question", hand.question.clone())
                .label(self.t.t("questionLabel"))
                .hint(self.t.t("questionHint"))
                .placeholder(self.t.t("questionPlaceholder"))
                .height(72.0)
                .on_input(Message::Question)
                .into(),
        ];
        if self.panes_denied {
            out.push(banner(Notice::Warning, self.t.t("panesDenied")).into());
        } else if self.agents.is_empty() {
            out.push(banner(Notice::Warning, self.t.t("noAgent")).into());
        } else if self.agents.len() > 1 {
            out.push(
                select(
                    self.agents
                        .iter()
                        .map(|agent| select_option(agent.name.clone(), agent.name.clone())),
                    target.clone(),
                )
                .label(self.t.t("agentLabel"))
                .on_select(Message::Choose)
                .into(),
            );
        }
        if let Some(problem) = &hand.problem {
            out.push(
                banner(
                    Notice::Error,
                    self.t.tv(
                        &format!("problem.{}", problem.code),
                        &[("agent", &problem.agent), ("detail", &problem.detail)],
                    ),
                )
                .into(),
            );
        }
        let can_send = !hand.question.trim().is_empty() && target.is_some() && !hand.sending;
        out.push(
            row![
                button(if hand.sending {
                    self.t.t("sending")
                } else {
                    self.t.t("send")
                })
                .small()
                .primary()
                .on_press_maybe(can_send.then_some(Message::SendHandOver)),
                button(self.t.t("cancel"))
                    .small()
                    .ghost()
                    .on_press(Message::CloseHandOver),
            ]
            .spacing(8.0)
            .into(),
        );
        Some(column(out).spacing(8.0).into())
    }
}

/// A value's text: in the terminals' face, at the size of code.
fn code(value: impl Into<String>) -> hangar_kit_sdk::ui::Text {
    text(value).mono().size(TextSize::Code)
}

/// The text a cell shows of a value: on one line, and cut to what the cell holds.
fn cell_text_of(value: &str, cut: bool) -> String {
    let line = logic::one_line(value);
    if line.chars().count() > CELL_SHOWN {
        let mut shown: String = line.chars().take(CELL_SHOWN).collect();
        shown.push('…');
        shown
    } else if cut {
        format!("{line}…")
    } else {
        line
    }
}

/// What a cell reads as, for sizing its column.
fn cell_text(cell: &DbCell, db: &Db) -> String {
    match cell {
        DbCell::Null => db.t.t("null"),
        DbCell::Blob { length } => db.t.tn("blob", *length as i64, &[]),
        DbCell::Integer(value) | DbCell::Real(value) => value.clone(),
        DbCell::Text { value, cut, .. } => cell_text_of(value, *cut),
    }
}
