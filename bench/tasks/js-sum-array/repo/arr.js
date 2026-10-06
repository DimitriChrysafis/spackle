function sum(xs) {
  let total;
  for (const x of xs) total += x;
  return total;
}
module.exports = { sum };
