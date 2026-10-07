import test from "node:test";
import assert from "node:assert/strict";
import { render } from "../src/index.js";
import { renderInline } from "../src/inline.js";

test("bold", () => {
  assert.equal(renderInline("a **b** c"), "a <strong>b</strong> c");
});

test("emphasis", () => {
  assert.equal(renderInline("a *b* c"), "a <em>b</em> c");
});

test("bold and em together", () => {
  assert.equal(renderInline("**b** and *e*"), "<strong>b</strong> and <em>e</em>");
});

test("code span", () => {
  assert.equal(renderInline("use `x++` now"), "use <code>x++</code> now");
});

test("link", () => {
  assert.equal(renderInline("[t](http://x)"), '<a href="http://x">t</a>');
});

test("escapes html", () => {
  assert.equal(renderInline("<b>&</b>"), "&lt;b&gt;&amp;&lt;/b&gt;");
});

test("headings and para", () => {
  const html = render("# T\n\nhello world\n");
  assert.equal(html, "<h1>T</h1>\n<p>hello world</p>\n");
});

test("list", () => {
  const html = render("- a\n- b\n");
  assert.equal(html, "<ul><li>a</li><li>b</li></ul>\n");
});
