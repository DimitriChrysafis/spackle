function format(iso) {
  const d = new Date(iso + "T00:00:00Z");
  return `${d.getUTCFullYear()}-${d.getUTCMonth()}-${d.getUTCDate()}`;
}
module.exports = { format };
