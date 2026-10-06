const { test } = require("node:test");
const assert = require("node:assert");
const { format } = require("./dates.js");

test("format", () => {
  assert.strictEqual(format("2024-03-15"), "2024-3-15");
  assert.strictEqual(format("2024-12-01"), "2024-12-1");
});
