/** Named entity table for the common HTML entities. */
export const ENTITIES = {
  amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: "\u00a0",
  copy: "\u00a9", hellip: "\u2026", mdash: "\u2014", ndash: "\u2013",
};

export function decodeEntities(s) {
  return s.replace(/&([a-z]+);/g, (m, name) => ENTITIES[name] ?? m);
}
