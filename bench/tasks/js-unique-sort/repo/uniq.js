function uniqueSorted(xs) {
  const out = [];
  for (const x of xs) {
    if (!out.includes(x)) out.push(x);
  }
  return out.sort();
}
module.exports = { uniqueSorted };
