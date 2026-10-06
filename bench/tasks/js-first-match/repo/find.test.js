const { test } = require("node:test");
const assert = require("node:assert");
const { findName } = require("./find.js");

test("findName substring", () => {
  const users = [{ name: "alice" }, { name: "bob" }];
  assert.deepStrictEqual(findName(users, "bo"), { name: "bob" });
  assert.strictEqual(findName(users, "z"), null);
});
