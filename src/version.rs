//! Central place for the game's version, sourced from `Cargo.toml` at compile
//! time so there is a single source of truth. Displayed on the loading screen
//! and in the HUD.

/// Semantic version string, e.g. `"0.1.0"`, taken from the `version` field in
/// `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Short human-facing build label shown on the loading screen.
pub fn build_label() -> String {
    format!("v{VERSION}")
}
