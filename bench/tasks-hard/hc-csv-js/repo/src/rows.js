/** Row-level helpers: column projection and filtering. */

export function pick(rows, cols) {
  return rows.map((r) => cols.map((c) => r[c]));
}

export function column(rows, i) {
  return rows.map((r) => r[i]);
}

export function transpose(rows) {
  if (rows.length === 0) return [];
  const w = Math.max(...rows.map((r) => r.length));
  const out = [];
  for (let c = 0; c < w; c++) {
    out.push(rows.map((r) => r[c]));
  }
  return out;
}
