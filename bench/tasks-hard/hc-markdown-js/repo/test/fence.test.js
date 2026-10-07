import test from "node:test";
import assert from "node:assert/strict";
import { splitFenced, isHr } from "../src/hr.js";

test("splits fenced code", () => {
  const parts = splitFenced("intro\n```\ncode\n```\noutro");
  assert.equal(parts[0].kind, "md");
  assert.equal(parts[1].kind, "code");
  assert.equal(parts[2].kind, "md");
});

test("hr detection", () => {
  assert.ok(isHr("---"));
  assert.ok(isHr("* * *"));
  assert.ok(!isHr("--"));
});
