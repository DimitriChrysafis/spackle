/** Horizontal rules and fenced-code passthrough. */

export function splitFenced(src) {
  // yields {kind:'code'|'md', text}
  const out = [];
  let inFence = false;
  let cur = [];
  for (const line of src.split("\n")) {
    if (line.trim().startsWith("```")) {
      out.push({ kind: inFence ? "code" : "md", text: cur.join("\n") });
      cur = [];
      inFence = !inFence;
      continue;
    }
    cur.push(line);
  }
  out.push({ kind: inFence ? "code" : "md", text: cur.join("\n") });
  return out.filter((b) => b.text !== "");
}

export function isHr(line) {
  return /^\s*([-*_])\s*(\1\s*){2,}$/.test(line);
}
