# urlp

URL splitting into scheme / userinfo / host / port / path / query /
fragment, plus `join()` relative resolution and query-string helpers.
Schemes require `://`; `host:port` alone is authority, not a scheme.
