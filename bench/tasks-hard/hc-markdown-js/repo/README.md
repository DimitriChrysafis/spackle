# mdlite

A ~100-line markdown renderer: `**bold**`, `*em*`, `` `code` ``, links,
`#`/`##`/`###` headings, `-` lists, `>` quotes, paragraphs.

```js
import { render } from "mdlite";
render("# Hi\n\nsome *text*\n");
```

Also ships `toc()` for heading extraction.
