import test from "node:test";
import assert from "node:assert/strict";
import { Emitter } from "../src/emitter.js";
import { pipe } from "../src/pipe.js";

test("forwards mapped events", () => {
  const a = new Emitter();
  const b = new Emitter();
  const seen = [];
  b.on("y", (v) => seen.push(v));
  pipe(a, b, (n, args) => ["y", args]);
  a.emit("x", 9);
  assert.deepEqual(seen, [9]);
});
