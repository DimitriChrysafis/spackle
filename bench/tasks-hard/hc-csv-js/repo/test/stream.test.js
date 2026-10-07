import test from "node:test";
import assert from "node:assert/strict";
import { CsvWriter } from "../src/stream.js";
import { parse } from "../src/csv.js";

test("writer buffers and emits", () => {
  const w = new CsvWriter();
  w.writeRow(["a", "b"]).writeRows([["1", "2"], ["3", "4"]]);
  assert.equal(w.length, 3);
  assert.equal(parse(w.toString()).length, 3);
});
