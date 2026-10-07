export { renderInline, escapeHtml } from "./inline.js";
export { renderBlocks } from "./block.js";

import { renderBlocks } from "./block.js";
export function render(src) {
  return renderBlocks(src) + "\n";
}
