"use strict";
// Records the answers the native tab's tests are given: the real script (tools/db.js), started the
// way Hangar starts it (the manifest's argv, its patterns, no shell), over the fixture database the
// Node tests build. The Rust tests play them back as `command.run`'s answers, so what the tab
// reads in them is what the script prints, not what the tab's author thought it would.
//
//   node native/tests/record.js           writes native/tests/fixtures/answers.json
//   node native/tests/record.js --check   fails if that file is not what the script prints now
//
// `test/native-fixtures.test.js` runs the check with the other Node tests. What varies from one
// machine to another (the driver, the files' sizes and times) is fixed before it is written.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { buildFixture, runDeclared, canBuildFixture } = require("../../test/helpers");

const FILE = path.join(__dirname, "fixtures", "answers.json");
const SIZE = 20480;
const MODIFIED = 1700000000000;

/** The key a test finds an answer by: the command, then its arguments by name. */
const key = (name, args = {}) =>
  [name, ...Object.keys(args).sort().map((arg) => `${arg}=${args[arg]}`)].join(" ");

const rows = (table, extra = {}) => ({ path: "app.db", table, sort: "0", direction: "asc", offset: "0", limit: "50", chars: "200", ...extra });
const whole = (table, extra = {}) => rows(table, { limit: "1", chars: "100000", ...extra });

/** What the first world records of `app.db`: its tables, and the pages the tests turn to. */
function ofApp() {
  const requests = [["tables", { path: "app.db" }]];
  for (const table of ["users", "events", "pairs", "empty_one", "active_users"]) {
    requests.push(["schema", { path: "app.db", table }], ["rows", rows(table)]);
  }
  for (const sort of ["1", "2", "3"]) {
    for (const direction of ["asc", "desc"]) requests.push(["rows", rows("events", { sort, direction })]);
  }
  for (const offset of ["50", "100"]) requests.push(["rows", rows("events", { offset })]);
  for (const direction of ["asc", "desc"]) requests.push(["rows", rows("users", { sort: "2", direction })]);
  // The whole of the 6 000 characters of Ada's bio, by position in the page, unsorted and sorted.
  requests.push(["rows", whole("users")], ["rows", whole("users", { sort: "2" })]);
  return requests;
}

const worlds = {
  // The only database there is.
  single: { files: ["app.db"], requests: [["list"], ...ofApp()] },
  // Two: an agent asks for the second, then for the first.
  pair: { files: ["app.db", "other.sqlite"], requests: [["list"], ["tables", { path: "app.db" }], ["tables", { path: "other.sqlite" }]] },
  stale: {
    files: ["app.db", "b.db"],
    requests: [
      ["list"],
      ["tables", { path: "app.db" }],
      ["tables", { path: "b.db" }],
      ["schema", { path: "app.db", table: "events" }],
      ["rows", rows("events")],
      ["rows", rows("events", { sort: "2" })],
    ],
  },
  // A name the patterns refuse is listed, as not openable.
  odd: { files: ["app.db", "bad ;name.db"], requests: [["list"], ["tables", { path: "app.db" }]] },
  empty: { files: [], requests: [["list"]] },
  // A file that is not a database, and one that is gone, which an agent may still ask for.
  broken: { files: ["app.db"], garbage: ["z.sqlite"], requests: [["list"], ["tables", { path: "z.sqlite" }], ["tables", { path: "missing.db" }]] },
  // No SQLite reader on the computer.
  nosqlite: { files: ["app.db"], env: { HANGAR_DB_DRIVER: "none" }, requests: [["list"], ["tables", { path: "app.db" }]] },
  // Another program is writing to it.
  locked: { files: ["app.db"], locked: true, requests: [["list"], ["tables", { path: "app.db" }]] },
};

function setUp(dir, world) {
  const source = path.join(dir, "source.db");
  buildFixture(source);
  // The script lists the newest first: give the files times that put them in the order they are
  // written in, so that the listing does not depend on how fast they were copied.
  world.files.forEach((name, index) => {
    fs.copyFileSync(source, path.join(dir, name));
    const time = MODIFIED / 1000 - index * 60;
    fs.utimesSync(path.join(dir, name), time, time);
  });
  fs.rmSync(source);
  for (const name of world.garbage || []) fs.writeFileSync(path.join(dir, name), "x".repeat(4096));
}

/** What the script printed, with what differs between machines fixed. */
function steady(name, run) {
  if (run.exitCode !== 0 || !run.stdout.trim()) return run;
  const document = JSON.parse(run.stdout);
  if (name === "list" && Array.isArray(document.files)) {
    for (const file of document.files) Object.assign(file, { size: SIZE, modified: MODIFIED });
  }
  if (name === "tables" && document.ok) Object.assign(document, { size: SIZE, driver: "node:sqlite" });
  return { ...run, stdout: `${JSON.stringify(document)}\n` };
}

/** Every world's answers, as the file holds them. */
function record() {
  const out = { worlds: {} };
  for (const [world, spec] of Object.entries(worlds)) {
    const dir = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "db-kit-record-")));
    let writer = null;
    try {
      setUp(dir, spec);
      if (spec.locked) {
        const { DatabaseSync } = require("node:sqlite");
        writer = new DatabaseSync(path.join(dir, "app.db"));
        writer.exec("BEGIN EXCLUSIVE");
      }
      const answers = {};
      for (const [name, args] of spec.requests) {
        const run = runDeclared(name, args, dir, spec.env || {});
        const { exitCode, stdout, stderr } = steady(name, run);
        answers[key(name, args)] = { exitCode, stdout, stderr };
      }
      out.worlds[world] = answers;
    } finally {
      if (writer) writer.close();
      fs.rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
    }
  }
  return out;
}

const text = (recorded) => `${JSON.stringify(recorded, null, 1)}\n`;

// The locked world needs node:sqlite for its writer, and every world a way to build the fixture.
const hasNodeSqlite = (() => {
  try {
    require("node:sqlite");
    return true;
  } catch {
    return false;
  }
})();

module.exports = { record, text, FILE, key, canRecord: canBuildFixture && hasNodeSqlite };

if (require.main === module) {
  const recorded = text(record());
  if (process.argv.includes("--check")) {
    if (fs.readFileSync(FILE, "utf8") !== recorded) {
      console.error("native/tests/fixtures/answers.json is not what tools/db.js prints now: run node native/tests/record.js");
      process.exit(1);
    }
  } else {
    fs.mkdirSync(path.dirname(FILE), { recursive: true });
    fs.writeFileSync(FILE, recorded);
    console.log(`${FILE}: ${Object.keys(JSON.parse(recorded).worlds).length} worlds`);
  }
}
