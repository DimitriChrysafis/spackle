/** Compile "/users/:id/edit" into a matcher over path segments. */

export function compile(pattern) {
  const segs = pattern.split("/").filter((s) => s.length > 0);
  const parts = segs.map((seg) => {
    if (seg.startsWith(":")) return { kind: "param", name: seg.slice(1) };
    if (seg === "*") return { kind: "wildcard" };
    return { kind: "static", value: seg };
  });
  return { pattern, parts };
}

export function match(compiled, path) {
  const segs = path.split("/").filter((s) => s.length > 0);
  const params = {};
  let i = 0;
  for (const part of compiled.parts) {
    if (part.kind === "wildcard") {
      params["*"] = segs.slice(i).join("/");
      return params;
    }
    if (i >= segs.length) return null;
    if (part.kind === "static") {
      if (segs[i] !== part.value) return null;
    } else {
      params[part.name] = decodeURIComponent(segs[i]);
    }
    i++;
  }
  return i === segs.length ? params : null;
}
