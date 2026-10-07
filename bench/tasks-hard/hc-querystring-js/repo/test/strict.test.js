import test from "node:test";
import assert from "node:assert/strict";
import { parseStrict } from "../src/strict.js";

test("strict rejects bad escapes", () => {
  assert.throws(() => parseStrict("a=%zz"));
  assert.throws(() => parseStrict("a=%2"));
});

test("strict accepts good input", () => {
  assert.deepEqual(parseStrict("a=%20"), { a: " " });
});
