//! Cross-type helpers shared by more than one diagram spec.
//!
//! Currently holds the edge role → stroke/label color dispatch used by both
//! `flowchart` and `architecture`. Kept in one place so the visual rule stays
//! in sync across types: a "primary" edge reads as accent anywhere, an
//! "external" edge always reads as link (blue), default reads as muted.

use crate::theme::{Color, Theme};
use crate::types::flowchart::EdgeRole;

/// Map an [`EdgeRole`] to its stroke / label color under the given theme.
///
/// * `Default`  → `palette.muted`  (secondary arrow stroke)
/// * `Primary`  → `palette.accent` (load-bearing / focal flow)
/// * `External` → `palette.link`   (cross-system, HTTP-style)
///
/// The returned color is used for both the arrow shaft *and* the matching
/// edge label so role reads consistently at a glance.
#[inline]
#[must_use]
pub(crate) fn edge_color_for_role(role: EdgeRole, theme: &Theme) -> Color {
    match role {
        EdgeRole::Default => theme.palette.muted,
        EdgeRole::Primary => theme.palette.accent,
        EdgeRole::External => theme.palette.link,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_role_maps_to_muted() {
        let t = Theme::light();
        assert_eq!(edge_color_for_role(EdgeRole::Default, &t), t.palette.muted);
    }

    #[test]
    fn primary_role_maps_to_accent() {
        let t = Theme::light();
        assert_eq!(edge_color_for_role(EdgeRole::Primary, &t), t.palette.accent);
    }

    #[test]
    fn external_role_maps_to_link() {
        let t = Theme::light();
        assert_eq!(edge_color_for_role(EdgeRole::External, &t), t.palette.link);
    }

    #[test]
    fn dark_theme_uses_dark_link() {
        // The helper must respect the theme's palette — swap to dark, link
        // must swap too.
        let d = Theme::dark();
        assert_eq!(edge_color_for_role(EdgeRole::External, &d), d.palette.link);
        assert_ne!(
            edge_color_for_role(EdgeRole::External, &d),
            Theme::light().palette.link
        );
    }
}
