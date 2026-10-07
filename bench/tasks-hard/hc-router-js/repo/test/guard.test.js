import test from "node:test";
import assert from "node:assert/strict";
import { sanitize, joinPaths } from "../src/guard.js";

test("normalize slashes and dots", () => {
  assert.equal(sanitize("//a/./b"), "/a/b");
});

test("dotdot pops", () => {
  assert.equal(sanitize("/a/../b"), "/b");
});

test("escape rejected", () => {
  assert.throws(() => sanitize("/../x"));
});

test("joinPaths", () => {
  assert.equal(joinPaths("/api/", "/v1"), "/api/v1");
});
