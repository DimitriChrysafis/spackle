import { Emitter } from "./emitter.js";

/** An emitter that remembers the last payload per event and replays it
 *  to listeners registered after the fact. */
export class ReplayEmitter extends Emitter {
  constructor() {
    super();
    this._last = new Map();
  }

  emit(name, ...args) {
    this._last.set(name, args);
    return super.emit(name, ...args);
  }

  on(name, fn) {
    super.on(name, fn);
    if (this._last.has(name)) fn(...this._last.get(name));
    return this;
  }
}
