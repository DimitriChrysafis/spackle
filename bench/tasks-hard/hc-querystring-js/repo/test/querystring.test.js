import test from "node:test";
import assert from "node:assert/strict";
import { parse, stringify } from "../src/querystring.js";

test("basic pairs", () => {
  assert.deepEqual(parse("a=1&b=2"), { a: "1", b: "2" });
});

test("leading ? tolerated", () => {
  assert.deepEqual(parse("?x=1"), { x: "1" });
});

test("repeated keys collect", () => {
  assert.deepEqual(parse("a=1&a=2&b=3"), { a: ["1", "2"], b: "3" });
});

test("plus is space", () => {
  assert.deepEqual(parse("q=a+b"), { q: "a b" });
});

test("percent decoding", () => {
  assert.deepEqual(parse("q=%26%3D"), { q: "&=" });
});

test("key without value", () => {
  assert.deepEqual(parse("flag"), { flag: "" });
});

test("round trip", () => {
  const obj = { a: "1", b: ["x", "y"], c: "a b" };
  assert.deepEqual(parse(stringify(obj)), { a: "1", b: ["x", "y"], c: "a b" });
});
