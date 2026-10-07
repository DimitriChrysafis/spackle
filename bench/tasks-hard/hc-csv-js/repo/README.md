# csvlite

CSV reader/writer with quoting, embedded commas/newlines, `""` escapes,
dialect presets (excel, tsv, semicolon), and `toObjects` row mapping.

```js
parse('"a,b",c\n')            // [["a,b","c"]]
stringify([["x","y,z"]])       // "x,\"y,z\"\n"
```
