/** Compose middleware fns around a final handler.
 *  each mw: (req, next) => response */

export function compose(middlewares, final) {
  return function (req) {
    let idx = -1;
    const dispatch = (i, req2) => {
      if (i <= idx) throw new Error("next() called twice");
      idx = i;
      const fn = i === middlewares.length ? final : middlewares[i];
      return fn(req2, (r) => dispatch(i + 1, r ?? req2));
    };
    return dispatch(0, req);
  };
}
