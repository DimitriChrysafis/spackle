/**
 * Tiny event emitter. `once` listeners fire a single time even when
 * re-emitted from inside a handler. emit() walks the current listener
 * list in registration order.
 */
export class Emitter {
  constructor() {
    this._handlers = new Map();
  }

  on(name, fn) {
    if (typeof fn !== "function") throw new TypeError("listener must be a function");
    if (!this._handlers.has(name)) this._handlers.set(name, []);
    this._handlers.get(name).push({ fn, once: false });
    return this;
  }

  once(name, fn) {
    if (typeof fn !== "function") throw new TypeError("listener must be a function");
    if (!this._handlers.has(name)) this._handlers.set(name, []);
    const entry = { fn, once: true };
    this._handlers.get(name).push(entry);
    return this;
  }

  off(name, fn) {
    const list = this._handlers.get(name);
    if (!list) return this;
    const i = list.findIndex((e) => e.fn === fn);
    if (i !== -1) list.splice(i, 1);
    if (list.length === 0) this._handlers.delete(name);
    return this;
  }

  emit(name, ...args) {
    const list = this._handlers.get(name);
    if (!list || list.length === 0) return false;
    for (let i = 0; i < list.length; i++) {
      const entry = list[i];
      if (entry.once) this.off(name, entry.fn);
      entry.fn(...args);
    }
    return true;
  }

  listenerCount(name) {
    const list = this._handlers.get(name);
    return list ? list.length : 0;
  }

  removeAll(name) {
    if (name === undefined) this._handlers.clear();
    else this._handlers.delete(name);
    return this;
  }

  eventNames() {
    return [...this._handlers.keys()];
  }
}
