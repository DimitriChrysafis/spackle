# tinyrouter

Path router with `:params`, `*` wildcards, percent-decoding, and a
middleware composer. Static segments must win over params.

```js
const r = new Router();
r.get("/users/:id", (p) => p.id);
r.handle("/users/7");   // -> { status: 200, body: "7" }
```
