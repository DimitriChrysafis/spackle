import test from "node:test";
import assert from "node:assert/strict";
import { pick, column, transpose } from "../src/rows.js";

test("pick", () => {
  assert.deepEqual(pick([["a","b","c"],["1","2","3"]], [0,2]),
                   [["a","c"],["1","3"]]);
});

test("column", () => {
  assert.deepEqual(column([["a","b"],["c","d"]], 1), ["b","d"]);
});

test("transpose", () => {
  assert.deepEqual(transpose([["a","b"],["c","d"]]), [["a","c"],["b","d"]]);
});
