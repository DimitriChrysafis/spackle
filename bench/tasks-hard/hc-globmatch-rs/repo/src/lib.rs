//! globmatch - compile shell-style globs into matchers.
//!
//! Supported: `*` (any run except `/`), `?` (one char except `/`),
//! `**/` (zero or more directories), `[abc]` char class, literals.

mod pattern;
pub use pattern::{compile, Matcher};
