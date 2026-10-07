import test from "node:test";
import assert from "node:assert/strict";
import { decodeEntities } from "../src/entities.js";

test("decodes known", () => {
  assert.equal(decodeEntities("a &amp; b"), "a & b");
  assert.equal(decodeEntities("&lt;x&gt;"), "<x>");
});

test("unknown left alone", () => {
  assert.equal(decodeEntities("&foo;"), "&foo;");
});
