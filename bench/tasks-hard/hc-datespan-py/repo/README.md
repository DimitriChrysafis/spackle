# spans

Half-open range algebra (`[lo, hi)`): `Span` for ints, `DateRange` for
ISO dates. `overlaps`, `merge`, `intersect`, `clamp`, `gap_days`, day
iteration, and fixed-size chunking.

Adjacent-but-not-overlapping ranges stay distinct on merge.
