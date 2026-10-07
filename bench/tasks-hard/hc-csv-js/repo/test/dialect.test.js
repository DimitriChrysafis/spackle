import test from "node:test";
import assert from "node:assert/strict";
import { resolveDialect, DIALECTS } from "../src/dialect.js";

test("presets exist", () => {
  assert.equal(DIALECTS.excel.delimiter, ",");
  assert.equal(DIALECTS.tsv.delimiter, "\t");
});

test("resolve by name", () => {
  assert.equal(resolveDialect("tsv").delimiter, "\t");
});

test("custom override", () => {
  const d = resolveDialect({ delimiter: "|" });
  assert.equal(d.delimiter, "|");
  assert.equal(d.quote, '"');
});

test("unknown throws", () => {
  assert.throws(() => resolveDialect("nope"));
});
