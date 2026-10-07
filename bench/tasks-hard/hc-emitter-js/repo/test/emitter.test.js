import test from "node:test";
import assert from "node:assert/strict";
import Emitter from "../src/index.js";

test("on + emit", () => {
  const e = new Emitter();
  const seen = [];
  e.on("x", (v) => seen.push(v));
  e.emit("x", 1);
  e.emit("x", 2);
  assert.deepEqual(seen, [1, 2]);
});

test("once fires exactly once", () => {
  const e = new Emitter();
  let n = 0;
  e.once("x", () => n++);
  e.emit("x");
  e.emit("x");
  assert.equal(n, 1);
});

test("once inside emit does not skip next listener", () => {
  const e = new Emitter();
  const seen = [];
  e.once("x", () => seen.push("once"));
  e.on("x", () => seen.push("always"));
  e.emit("x");
  assert.deepEqual(seen, ["once", "always"]);
});

test("emit during emit", () => {
  const e = new Emitter();
  const order = [];
  e.on("a", () => { order.push("a1"); e.emit("b"); });
  e.on("a", () => order.push("a2"));
  e.on("b", () => order.push("b1"));
  e.emit("a");
  assert.deepEqual(order, ["a1", "b1", "a2"]);
});

test("off removes listener", () => {
  const e = new Emitter();
  let n = 0;
  const fn = () => n++;
  e.on("x", fn);
  e.off("x", fn);
  e.emit("x");
  assert.equal(n, 0);
});

test("listenerCount and eventNames", () => {
  const e = new Emitter();
  e.on("a", () => {});
  e.on("b", () => {});
  e.once("b", () => {});
  assert.equal(e.listenerCount("b"), 2);
  assert.deepEqual(e.eventNames(), ["a", "b"]);
});
