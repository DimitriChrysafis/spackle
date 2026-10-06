#![forbid(unsafe_code)]

use spackle_core::PRODUCT_NAME;

#[must_use]
pub fn startup_banner() -> String {
    format!("{PRODUCT_NAME} — local llama.cpp coding agent")
}
