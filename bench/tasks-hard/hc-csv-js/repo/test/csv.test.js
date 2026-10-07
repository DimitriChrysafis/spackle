import test from "node:test";
import assert from "node:assert/strict";
import { parse, stringify, toObjects } from "../src/csv.js";

test("plain rows", () => {
  assert.deepEqual(parse("a,b,c\n1,2,3\n"), [["a","b","c"],["1","2","3"]]);
});

test("quoted comma", () => {
  assert.deepEqual(parse('"x,y",z\n'), [["x,y","z"]]);
});

test("quoted newline", () => {
  const rows = parse('"line1\nline2",b\n');
  assert.equal(rows.length, 1);
  assert.equal(rows[0][0], "line1\nline2");
});

test("escaped quote", () => {
  assert.deepEqual(parse('"she said ""hi""",x\n'), [['she said "hi"', "x"]]);
});

test("crlf tolerated", () => {
  const rows = parse("a,b\r\n1,2\r\n");
  assert.deepEqual(rows[0], ["a", "b"]);
  assert.deepEqual(rows[1], ["1", "2"]);
});

test("round trip", () => {
  const rows = [["a,b", "c"], ['q"q', "d"]];
  assert.deepEqual(parse(stringify(rows)), rows);
});

test("toObjects", () => {
  const objs = toObjects("name,age\nal,30\nbo,25\n");
  assert.deepEqual(objs[0], { name: "al", age: "30" });
  assert.equal(objs.length, 2);
});
