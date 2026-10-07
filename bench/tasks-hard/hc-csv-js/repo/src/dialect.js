/** Dialects: delimiter and quoting rules presets. */

export const DIALECTS = {
  excel: { delimiter: ",", quote: '"', newline: "\r\n" },
  tsv: { delimiter: "\t", quote: '"', newline: "\n" },
  semicolon: { delimiter: ";", quote: '"', newline: "\n" },
};

export function resolveDialect(nameOrObj) {
  if (typeof nameOrObj === "string") {
    const d = DIALECTS[nameOrObj];
    if (!d) throw new Error(`unknown dialect ${nameOrObj}`);
    return d;
  }
  return { ...DIALECTS.excel, ...nameOrObj };
}
