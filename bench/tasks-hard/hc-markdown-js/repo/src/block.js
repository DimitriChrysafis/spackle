import { renderInline } from "./inline.js";

/** Block-level markdown: # h1, ## h2, ### h3, - list, > quote, para. */
export function renderBlocks(src) {
  const lines = src.split("\n");
  const out = [];
  let para = [];
  let list = null;

  const flushPara = () => {
    if (para.length) {
      out.push("<p>" + renderInline(para.join(" ")) + "</p>");
      para = [];
    }
  };
  const flushList = () => {
    if (list) {
      out.push("<ul>" + list.map((i) => "<li>" + renderInline(i) + "</li>").join("") + "</ul>");
      list = null;
    }
  };

  for (const raw of lines) {
    const line = raw.trimEnd();
    if (line === "") { flushPara(); flushList(); continue; }
    if (line.startsWith("### ")) { flushPara(); flushList(); out.push("<h3>" + renderInline(line.slice(4)) + "</h3>"); continue; }
    if (line.startsWith("## ")) { flushPara(); flushList(); out.push("<h2>" + renderInline(line.slice(3)) + "</h2>"); continue; }
    if (line.startsWith("# ")) { flushPara(); flushList(); out.push("<h1>" + renderInline(line.slice(2)) + "</h1>"); continue; }
    if (line.startsWith("- ")) { flushPara(); (list ||= []).push(line.slice(2)); continue; }
    if (line.startsWith("> ")) { flushPara(); flushList(); out.push("<blockquote>" + renderInline(line.slice(2)) + "</blockquote>"); continue; }
    para.push(line);
  }
  flushPara();
  flushList();
  return out.join("\n");
}
