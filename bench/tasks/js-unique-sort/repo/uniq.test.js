const { test } = require("node:test");
const assert = require("node:assert");
const { uniqueSorted } = require("./uniq.js");

test("uniqueSorted", () => {
  assert.deepStrictEqual(uniqueSorted([3, 1, 3, 2]), [1, 2, 3]);
  assert.deepStrictEqual(uniqueSorted([10, 2, 2]), [2, 10]);
});
