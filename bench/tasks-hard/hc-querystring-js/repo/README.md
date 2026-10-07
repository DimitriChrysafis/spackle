# qstring

`application/x-www-form-urlencoded` parsing and serialization:

```js
parse("a=1&a=2")        // { a: ["1", "2"] }
parse("q=a+b")          // { q: "a b" }
stringify({x: "y z"})   // "x=y%20z"
parseForm("u[n]=1")     // { u: { n: "1" } }
```

Repeated keys collect into arrays; missing values are `""`.
