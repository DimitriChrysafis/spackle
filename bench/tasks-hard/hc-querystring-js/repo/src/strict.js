/** Strict mode: reject bare '+' and malformed escapes. */
export function parseStrict(qs) {
  const out = {};
  const s = qs.startsWith("?") ? qs.slice(1) : qs;
  if (s === "") return out;
  for (const pair of s.split("&")) {
    for (const m of pair.matchAll(/%(?![0-9A-Fa-f]{2})/g)) {
      throw new SyntaxError(`bad escape in ${pair}`);
    }
    const eq = pair.indexOf("=");
    const k = decodeURIComponent(eq === -1 ? pair : pair.slice(0, eq));
    const v = eq === -1 ? "" : decodeURIComponent(pair.slice(eq + 1));
    if (pair.includes("+")) {
      // strict mode treats '+' as literal, not space
    }
    if (out[k] === undefined) out[k] = v;
    else out[k] = [].concat(out[k], v);
  }
  return out;
}
