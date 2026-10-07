/** flat <-> nested conversion for bracket-style keys. */
import { parseForm } from "./form.js";

export function flatten(obj, prefix = "") {
  const out = {};
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}[${k}]` : k;
    if (v && typeof v === "object" && !Array.isArray(v)) {
      Object.assign(out, flatten(v, key));
    } else if (Array.isArray(v)) {
      v.forEach((item, i) => { out[`${key}[${i}]`] = item; });
    } else {
      out[key] = v;
    }
  }
  return out;
}
