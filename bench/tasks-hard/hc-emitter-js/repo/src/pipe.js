/** Forward all events from one emitter into another with a mapper. */

export function pipe(src, dst, map = (n, a) => [n, a]) {
  const orig = src.emit.bind(src);
  src.emit = (name, ...args) => {
    const res = orig(name, ...args);
    const [n2, a2] = map(name, args);
    dst.emit(n2, ...a2);
    return res;
  };
  return src;
}
