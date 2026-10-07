import test from "node:test";
import assert from "node:assert/strict";
import { Router } from "../src/router.js";

function make() {
  const r = new Router();
  r.get("/users/:id", (p) => `user:${p.id}`);
  r.get("/users/new", () => "new-form");
  r.get("/users/:id/posts/:pid", (p) => `post:${p.id}/${p.pid}`);
  r.get("/files/*", (p) => `file:${p["*"]}`);
  r.get("/", () => "root");
  return r;
}

test("static beats param", () => {
  const r = make();
  assert.equal(r.handle("/users/new").body, "new-form");
});

test("params captured", () => {
  const r = make();
  assert.equal(r.handle("/users/42").body, "user:42");
  assert.equal(r.handle("/users/7/posts/9").body, "post:7/9");
});

test("wildcard", () => {
  const r = make();
  assert.equal(r.handle("/files/a/b/c.txt").body, "file:a/b/c.txt");
});

test("no match -> 404", () => {
  const r = make();
  assert.equal(r.handle("/nope").status, 404);
});

test("root", () => {
  const r = make();
  assert.equal(r.handle("/").body, "root");
});

test("url-encoded param", () => {
  const r = new Router();
  r.get("/q/:term", (p) => p.term);
  assert.equal(r.handle("/q/a%20b").body, "a b");
});
