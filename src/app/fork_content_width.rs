//! [fork] adaptive content width: the transcript column and the composer
//! widen with the chat area instead of stopping at `CONTENT_MAX_WIDTH`.

use super::*;

/// Content width for a chat area of `chat_width`. Up to `CONTENT_MAX_WIDTH`
/// this matches upstream; past it the column fills the chat area, keeping only
/// the navigation rail's gutter on each side, so the rail shows at exactly the
/// widths it did before.
pub(super) fn adaptive_content_max_width(chat_width: f32) -> f32 {
    let gutter = NAVIGATION_RAIL_LEFT + NAVIGATION_RAIL_WIDTH + NAVIGATION_RAIL_CONTENT_GAP;
    (chat_width - gutter * 2.0).max(CONTENT_MAX_WIDTH)
}

impl Waku {
    /// Recompute the content width once per root frame, before the pane
    /// islands render. Uses the panels' layout widths rather than their
    /// mid-slide widths, so a slide does not reflow the transcript.
    pub(super) fn sync_content_max_width(&mut self, window: &Window) {
        let (sidebar_width, right_panel_width) = self.effective_panel_widths(window);
        let chat_width =
            f32::from(window.viewport_size().width) - sidebar_width - right_panel_width;
        self.content_max_width = adaptive_content_max_width(chat_width);
    }
}

#[cfg(test)]
mod fork_tests {
    use super::*;

    #[test]
    fn adaptive_content_width_matches_upstream_until_the_rail_gutter_fits() {
        assert_eq!(adaptive_content_max_width(600.0), CONTENT_MAX_WIDTH);
        assert_eq!(adaptive_content_max_width(872.0), CONTENT_MAX_WIDTH);
        assert_eq!(adaptive_content_max_width(1000.0), 848.0);
        assert_eq!(adaptive_content_max_width(2000.0), 1848.0);
    }
}
