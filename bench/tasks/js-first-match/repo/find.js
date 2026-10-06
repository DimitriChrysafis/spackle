function findName(users, fragment) {
  const u = users.find((u) => u.name === fragment);
  return u || null;
}
module.exports = { findName };
