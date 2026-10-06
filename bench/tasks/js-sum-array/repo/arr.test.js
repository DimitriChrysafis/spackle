const { test } = require("node:test");
const assert = require("node:assert");
const { sum } = require("./arr.js");

test("sum", () => {
  assert.strictEqual(sum([1, 2, 3]), 6);
  assert.strictEqual(sum([]), 0);
});
