import { compile, match } from "./pattern.js";

/** Insertion-ordered router. First registered match wins. */
export class Router {
  constructor() {
    this.routes = [];
  }

  add(pattern, handler) {
    this.routes.push({ compiled: compile(pattern), handler });
    return this;
  }

  get(pattern, handler) { return this.add(pattern, handler); }

  dispatch(path) {
    for (const route of this.routes) {
      const params = match(route.compiled, path);
      if (params !== null) {
        return { handler: route.handler, params };
      }
    }
    return null;
  }

  handle(path, req = {}) {
    const hit = this.dispatch(path);
    if (!hit) return { status: 404 };
    const params = Object.fromEntries(
      Object.entries(hit.params).map(([k, v]) => [k, v])
    );
    return { status: 200, body: hit.handler(params, req) };
  }
}
