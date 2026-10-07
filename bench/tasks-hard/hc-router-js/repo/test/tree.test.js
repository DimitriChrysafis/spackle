import test from "node:test";
import assert from "node:assert/strict";
import { describe } from "../src/tree.js";

test("describes patterns", () => {
  assert.equal(describe("/users/:id"), "users/:id");
  assert.equal(describe("/files/*"), "files/*");
  assert.equal(describe("/"), "");
});
