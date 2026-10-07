/** Inline markdown: **bold**, *em*, `code`, [text](href). */

export function renderInline(text) {
  let out = escapeHtml(text);
  out = out.replace(/`([^`]+)`/g, (m, code) => "`code`" + code + "`/code`");
  out = out.replace(/\*([^*]+)\*/g, "<em>$1</em>");
  out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  out = out.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2">$1</a>');
  // undo the code placeholder hack
  out = out.replace(/`code`([^]*)`\/code`/g, (m, c) => "<code>" + c + "</code>");
  return out;
}

export function escapeHtml(s) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
