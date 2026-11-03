"use strict";
// The native tab's tests (native/tests) are played the answers of the real script, recorded in
// native/tests/fixtures/answers.json by native/tests/record.js. This holds the recording to the script:
// when the script's output changes, the recording must be made again and the Rust tests run over it.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const { record, text, FILE, canRecord } = require("../native/tests/record");

test("the answers the native tab's tests are played are what the script prints now", { skip: canRecord ? false : "node:sqlite is not here to record with" }, () => {
  assert.equal(text(record()), fs.readFileSync(FILE, "utf8"), "run: node native/tests/record.js");
});
