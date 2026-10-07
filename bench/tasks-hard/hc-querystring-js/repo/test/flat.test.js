import test from "node:test";
import assert from "node:assert/strict";
import { flatten } from "../src/flat.js";
import { stringify, parse } from "../src/querystring.js";

test("flatten nested", () => {
  assert.deepEqual(flatten({u: {n: "al"}}), {"u[n]": "al"});
});

test("flatten arrays", () => {
  assert.deepEqual(flatten({t: ["a", "b"]}), {"t[0]": "a", "t[1]": "b"});
});

test("flat stays flat", () => {
  assert.deepEqual(flatten({x: "1"}), {x: "1"});
});
