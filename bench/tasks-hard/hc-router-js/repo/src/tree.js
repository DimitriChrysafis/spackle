/** Route table dumping for debugging. */
import { compile } from "./pattern.js";

export function describe(pattern) {
  const c = compile(pattern);
  return c.parts
    .map((p) => p.kind === "param" ? `:${p.name}` : p.kind === "wildcard" ? "*" : p.value)
    .join("/");
}
