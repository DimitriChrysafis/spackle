import test from "node:test";
import assert from "node:assert/strict";
import { toc } from "../src/toc.js";

test("extracts headings", () => {
  const t = toc("# A\ntext\n## B\n");
  assert.deepEqual(t, [{ level: 1, title: "A" }, { level: 2, title: "B" }]);
});

test("no headings", () => {
  assert.deepEqual(toc("just text\n"), []);
});
