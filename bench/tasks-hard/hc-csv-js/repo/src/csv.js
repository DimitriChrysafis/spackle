/** CSV reader supporting quoted fields, embedded commas/newlines,
 *  and "" escapes. parse(text) -> array of row arrays. */

export function parse(text) {
  const rows = [];
  for (const line of text.split("\n")) {
    if (line === "") continue;
    rows.push(parseLine(line));
  }
  return rows;
}

function parseLine(line) {
  const fields = [];
  let cur = "";
  let inQuotes = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (inQuotes) {
      if (ch === '"') {
        if (line[i + 1] === '"') { cur += '"'; i++; }
        else inQuotes = false;
      } else {
        cur += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ",") {
      fields.push(cur);
      cur = "";
    } else {
      cur += ch;
    }
  }
  fields.push(cur);
  return fields;
}

export function stringify(rows) {
  return rows
    .map((row) => row.map(quoteField).join(","))
    .join("\n") + "\n";
}

function quoteField(field) {
  const s = String(field);
  if (/[",\n]/.test(s)) return '"' + s.replace(/"/g, '""') + '"';
  return s;
}

export function toObjects(text) {
  const rows = parse(text);
  if (rows.length === 0) return [];
  const [head, ...rest] = rows;
  return rest.map((r) => Object.fromEntries(head.map((h, i) => [h, r[i]])));
}
