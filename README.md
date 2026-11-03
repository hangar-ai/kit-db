# Database

A Hangar kit that lets you look at what an agent's code actually wrote, without leaving Hangar:
a SQLite database of the project, opened **read-only**.

## What it does

**A tab**, « Base de données »:

- **Picks a database.** It finds the `.db`, `.sqlite`, `.sqlite3` and `.db3` files of the workspace
  folder (checked by their header, so a `.db` that is not SQLite is left out), or opens the one an agent
  asked for with `kit_tab_open { kit: "db", tab: "db", params: { path } }`. It remembers your last pick.
- **Lists the tables** (and views) as rows, each with its row count.
- **Shows a table**: its structure (columns, types, keys, defaults, indexes) and its rows, 50 at a
  time, sortable by a click on a column (ascending, descending, none), with previous and next.
- **Cuts long values** at 200 characters and shows the whole one (up to 100 000) when you ask. A blob
  is shown by its size, never its content.
- **Hands a table to an agent** only when you click **Confier à un agent**: it sends the database's
  path, the table's name, its columns and your question, **never a row**. Values may be secrets: they
  stay on your screen, and a test fails if one of the planted secrets of the fixture reaches an agent.

Designed states: no database found, database locked, no SQLite reader on the computer, command not
allowed, not a SQLite file, file gone, a command that failed or timed out, and a send refused. English
and French follow Hangar's language.

**A skill** that tells agents to open the tab rather than paste rows into the chat, and to treat what
they read as possibly secret.

## What it runs

Four declared commands, all `node {kitDir}/tools/db.js` with a **fixed verb** in the argv. Hangar shows
them verbatim when you enable the kit; only the parts in braces change, and each must match its rule.

| Command  | Runs                                                                         | Reads                                         |
| -------- | ---------------------------------------------------------------------------- | --------------------------------------------- |
| `list`   | `node {kitDir}/tools/db.js list`                                             | the names of the SQLite files of the workspace |
| `tables` | `node {kitDir}/tools/db.js tables {path}`                                    | table names and counts                        |
| `schema` | `node {kitDir}/tools/db.js schema {path} {table}`                            | columns and indexes of one table              |
| `rows`   | `node {kitDir}/tools/db.js rows {path} {table} {sort} {direction} {offset} {limit} {chars}` | one page of one table, 200 rows at most |

- **Read-only, always.** The script reaches the file with `node:sqlite` when the Node version has it
  (22.12 or later), opened with `readOnly: true` and `PRAGMA query_only`, and otherwise with the
  `sqlite3` program (3.33 or later) started with `-readonly` and no shell. A test opens the fixture
  both ways, tries every kind of write, including the pragma that lifts `query_only`, and checks the file
  is byte for byte what it was.
- **No free SQL.** There is no verb that takes SQL. The statements are fixed text; what fills them is a
  path, a table name, a column's number, `asc` or `desc` and numbers, each held to a strict pattern by
  Hangar and again by the script. A table must exist in the database and not be one of SQLite's own.
- **Inside the workspace.** `path` is relative to the folder the command starts in, may not contain
  `..`, and is resolved through links: a file outside the folder is refused. The file must start with
  SQLite's header.
- **Capped.** At most 200 rows per page and 100 000 characters per value, and a budget for the whole
  answer so it stays under the 1 MB Hangar keeps of a command's output. Text is cut in the database,
  so a huge value never leaves it whole.
- **The tab** holds `command:list`, `command:tables`, `command:schema` and `command:rows` (to run
  those), `files.read` (to look at the workspace's files), `data.read` and `data.write` (to keep your
  last pick in the kit's data folder, `state.json`), `panes.read` (to list the agents) and
  `panes.send` (to hand a table over, when you click).

## What it does not do

- It does not write, ever, and it cannot run a query of your own: for that, use your own tools.
- A table or view whose name is outside letters, digits and `_` (or starts with a digit) is listed but
  not opened.
- A database in WAL mode that a program is writing may answer « locked »; try again later.
- Encrypted databases (SQLCipher) are not SQLite files to the reader and are reported as such.

## The native tab

The tab exists twice. **Hangar 1.x** opens the page, `tabs/db/`, HTML, as it always did. **The native
Hangar** opens `tabs/db.wasm`: the same tab written in Rust (`native/`), compiled to WebAssembly and drawn
by Hangar itself with its own widgets, in your theme, with no browser engine. The manifest stays at
`"schema": 1`, keeps the HTML `entry` and adds `"native": "tabs/db.wasm"`: an older Hangar ignores the
field and opens the page, and the native Hangar opens the page only when the module cannot be loaded
(the tab's bar says so).

It does what the page does, with the same four commands, the same permissions and the same calls to
Hangar (`command.run`, `data.read`, `data.write`, `panes.list`, `panes.send`, `tab.setTitle`;
`files.read` stays declared and goes unused, as in the page). Values stay on screen: no cell is ever
logged, toasted or sent, and a test checks it over everything the tab calls.

Where it differs:

- **A singleton asked again for another database opens it.** `kit_tab_open { kit: "db", tab: "db",
  params: { path } }` on a tab already open sends it a `tab.params` event whose data is `{ params,
  caller }`; the page read `event.data.params.path`, which is never there, so it never heard the new
  path (it still does not: the page is left as it was). A missing Node.js reads as its own message
  too, where the page showed a generic failure.
- **A long value is read in a panel under the table.** Press a row: each of its cut texts offers
  « Show all N characters », which fills a text area that can be selected and copied. A cell itself is
  one line, cut at its column, with the start of the value in its tooltip.
- **Only the first 40 columns of a wide table are drawn**, with a note, where the page drew 200: a view
  is a message of at most 4 MB.
- **A cell cannot be selected with the pointer**, nor can the table's header stay in sight while its
  rows scroll: Hangar's widgets have no text selection yet, and the header is part of what scrolls.
  The table is at most 360 points tall, not 60 % of the window, and scrolls sideways when it is wide.
- No accessibility roles (`aria-sort` and the like): Hangar's widgets have none yet, so icons and
  glyphs are named by tooltips.

### Building it

```sh
scripts/build-native.sh           # builds native/ to WebAssembly, writes tabs/db.wasm
scripts/build-native.sh --check   # builds it, and fails if tabs/db.wasm is not what the source builds
cargo test --manifest-path native/Cargo.toml
```

Hangar runs the `.wasm` a kit ships and never builds one, so the module is committed, in the same commit
as the source that changes it. Needs [rustup](https://rustup.rs): `rust-toolchain.toml` pins the compiler
and the `wasm32-unknown-unknown` target, because the same source gives the same bytes only from the same
compiler (the build also maps the paths of your machine away, so it is the same from any folder).
`native/target/` is ignored.

- `native/src/logic.rs` is `logic.js`: the script's documents checked, the patterns, the brief an agent
  gets. `native/src/strings.rs` holds the English and French words, `native/src/lib.rs` is
  `controller.js` (the state, its messages) and `native/src/view.rs` is `view.js`.
- `native/tests/` are the page's tests, ported: the SDK's harness drives the tab, and a pretend Hangar
  answers it as Hangar does (every call held to this manifest's permissions and patterns) with what the
  real script printed, recorded in `native/tests/fixtures/answers.json` by
  `node native/tests/record.js`. `test/native-fixtures.test.js` keeps that file equal to what the script
  prints now.

## Tests and fixtures

`node --test kits/db/` for the script, the page and the manifest, and `cargo test --manifest-path
native/Cargo.toml` for the native tab (above). The fixture database is built in a temp folder by each test, with
`node:sqlite` or else the `sqlite3` program; the tests that need the `sqlite3` program are skipped where
it is not installed. The command runner of the tests applies the manifest's patterns the way Hangar does
and starts the real script.

## Get it

Link `kits/db` from Hangar, or clone <https://github.com/hangar-ai/kit-db>. Needs
Hangar 1.9.0 or later and Node.js on the PATH. The native tab is for the Hangar that has native tabs; every
other Hangar opens the page.

## This repository

`node --test` runs the tests, with Node alone. These tests need nothing from Hangar's repository. The
native tab's tests (`cargo test`) need Rust, and the SDK, which they fetch from Hangar's repository.

CI calls [kit-check](https://github.com/hangar-ai/.github/blob/main/.github/workflows/kit-check.yml), and
builds and tests the native tab, failing if the committed `tabs/db.wasm` is not what the source builds. A release is
a tag, `v` and the manifest's version, and it reaches Hangar's gallery once
[the catalog](https://github.com/hangar-ai/catalog) is updated and reviewed.
