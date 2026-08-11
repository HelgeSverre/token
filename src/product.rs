//! Canonical product identity shared by Token's runtime and platform integrations.

/// User-facing application name. The command-line executable remains `token`.
pub const DISPLAY_NAME: &str = "Token";
/// Stable reverse-DNS identifier used by desktop packages.
pub const IDENTIFIER: &str = "no.helgesverre.token";
/// Product homepage exposed by package managers.
pub const HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");
/// Human-readable product description.
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
/// Release version used by native package metadata and About panels.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_identity_is_stable() {
        assert_eq!(DISPLAY_NAME, "Token");
        assert_eq!(IDENTIFIER, "no.helgesverre.token");
        assert!(!VERSION.is_empty());
        assert!(HOMEPAGE.starts_with("https://"));
        assert!(!DESCRIPTION.is_empty());
    }
}
