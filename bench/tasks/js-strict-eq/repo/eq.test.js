const { test } = require("node:test");
const assert = require("node:assert");
const { same } = require("./eq.js");

test("same", () => {
  assert.strictEqual(same("1", 1), false);
  assert.strictEqual(same(2, 2), true);
});
