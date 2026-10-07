/** form encoding helpers: a[b]=1 style nested keys stay flat but
 *  bracket names are preserved verbatim. */

import { parse } from "./querystring.js";

export function parseForm(qs) {
  const flat = parse(qs);
  const out = {};
  for (const [k, v] of Object.entries(flat)) {
    const m = k.match(/^([^\[]+)\[([^\]]*)\]$/);
    if (m) {
      const [, head, sub] = m;
      if (!out[head] || typeof out[head] !== "object") out[head] = {};
      if (sub === "") {
        out[head] = Array.isArray(out[head]) ? out[head] : [];
        out[head].push(v);
      } else {
        out[head][sub] = v;
      }
    } else {
      out[k] = v;
    }
  }
  return out;
}
