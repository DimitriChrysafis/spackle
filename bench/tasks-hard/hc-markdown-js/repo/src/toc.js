/** Extract a table of contents from markdown source. */
export function toc(src) {
  const out = [];
  for (const line of src.split("\n")) {
    const m = line.match(/^(#{1,3})\s+(.+)$/);
    if (m) out.push({ level: m[1].length, title: m[2].trim() });
  }
  return out;
}
