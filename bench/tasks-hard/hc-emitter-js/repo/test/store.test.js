import test from "node:test";
import assert from "node:assert/strict";
import { ReplayEmitter } from "../src/store.js";

test("late listener gets last payload", () => {
  const e = new ReplayEmitter();
  e.emit("ready", 42);
  let got = null;
  e.on("ready", (v) => { got = v; });
  assert.equal(got, 42);
});

test("normal emit still works", () => {
  const e = new ReplayEmitter();
  const seen = [];
  e.on("x", (v) => seen.push(v));
  e.emit("x", 1);
  e.emit("x", 2);
  assert.deepEqual(seen, [1, 2]);
});
