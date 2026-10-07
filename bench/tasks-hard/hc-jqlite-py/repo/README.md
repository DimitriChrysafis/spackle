# jql

A tiny jq subset: `.`, `.a.b`, `.arr[0]`, `.arr[]`, `.obj[]`, `a | b`,
optional `?` steps, plus builtins (`length`, `keys`, `has`, `first`,
`last`).

```py
run(".items[].id", doc)   # [1, 2, 3]
```
