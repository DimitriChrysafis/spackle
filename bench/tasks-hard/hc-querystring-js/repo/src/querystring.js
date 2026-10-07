/** Parse/serialize application/x-www-form-urlencoded query strings.
 *  Repeated keys collect into arrays. '+' is a literal plus inside
 *  names/values produced by encodeURIComponent, and "%20" decodes
 *  to space. */

export function parse(qs) {
  const out = {};
  const s = qs.startsWith("?") ? qs.slice(1) : qs;
  if (s === "") return out;
  for (const pair of s.split("&")) {
    if (pair === "") continue;
    const eq = pair.indexOf("=");
    const rawK = eq === -1 ? pair : pair.slice(0, eq);
    const rawV = eq === -1 ? "" : pair.slice(eq + 1);
    const k = decodeURIComponent(rawK);
    const v = decodeURIComponent(rawV);
    if (out[k] === undefined) out[k] = v;
  }
  return out;
}

export function stringify(obj) {
  const parts = [];
  for (const [k, v] of Object.entries(obj)) {
    if (v === undefined) continue;
    const vals = Array.isArray(v) ? v : [v];
    for (const item of vals) {
      parts.push(
        encodeURIComponent(k) + "=" + encodeURIComponent(String(item))
      );
    }
  }
  return parts.join("&");
}
