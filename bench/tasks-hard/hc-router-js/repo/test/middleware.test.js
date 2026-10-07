import test from "node:test";
import assert from "node:assert/strict";
import { compose } from "../src/middleware.js";

test("middleware order", async () => {
  const order = [];
  const mw1 = async (req, next) => { order.push("m1-in"); const r = await next(req); order.push("m1-out"); return r; };
  const mw2 = async (req, next) => { order.push("m2-in"); const r = await next(req); order.push("m2-out"); return r; };
  const final = async (req) => { order.push("final"); return "done"; };
  const h = compose([mw1, mw2], final);
  const res = await h({});
  assert.equal(res, "done");
  assert.deepEqual(order, ["m1-in", "m2-in", "final", "m2-out", "m1-out"]);
});

test("double next throws", async () => {
  const bad = async (req, next) => { await next(req); await next(req); };
  await assert.rejects(() => compose([bad], async () => "x")({}));
});
