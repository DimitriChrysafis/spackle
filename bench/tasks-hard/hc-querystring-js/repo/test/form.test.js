import test from "node:test";
import assert from "node:assert/strict";
import { parseForm } from "../src/form.js";

test("bracket nesting", () => {
  assert.deepEqual(parseForm("user[name]=al"), { user: { name: "al" } });
});

test("empty brackets append", () => {
  assert.deepEqual(parseForm("tags[]=a&tags[]=b"), { tags: ["a", "b"] });
});

test("plain keys untouched", () => {
  assert.deepEqual(parseForm("x=1"), { x: "1" });
});
