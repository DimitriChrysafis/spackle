/** Path hygiene: reject traversal and normalize slashes. */

export function sanitize(path) {
  const segs = path.split("/").filter((s) => s && s !== ".");
  const out = [];
  for (const s of segs) {
    if (s === "..") {
      if (out.length === 0) throw new Error("path escapes root");
      out.pop();
    } else {
      out.push(s);
    }
  }
  return "/" + out.join("/");
}

export function joinPaths(a, b) {
  return sanitize(a.replace(/\/$/, "") + "/" + b.replace(/^\//, ""));
}
