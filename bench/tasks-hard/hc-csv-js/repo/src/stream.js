/** Chunk-fed CSV writer: buffers rows, flushes as text. */
import { stringify } from "./csv.js";

export class CsvWriter {
  constructor() {
    this.rows = [];
  }

  writeRow(row) {
    this.rows.push(row);
    return this;
  }

  writeRows(rows) {
    for (const r of rows) this.rows.push(r);
    return this;
  }

  toString() {
    return stringify(this.rows);
  }

  get length() {
    return this.rows.length;
  }
}
