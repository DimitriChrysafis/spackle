# limiter

Three limiter strategies sharing a wall-clock API (`now` seconds):

- `RateLimiter` — sliding-window log, allows `limit` calls / window
- `TokenBucket` — smooth shaping with capacity + refill rate
- `FixedWindow` — per-bucket counter, cheapest

All return booleans from `allow`/`take` and expose `remaining`/`usage`.
