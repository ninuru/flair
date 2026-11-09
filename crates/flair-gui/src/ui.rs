use flair_core::theme::Rgba;

pub const BAR: f32 = 34.0;
pub const LINE: f32 = 1.0;
pub const INSET: f32 = 8.0;
pub const ITEM_HEIGHT: f32 = 24.0;
pub const ITEM_RADIUS: f32 = 6.0;
pub const ITEM_PAD: f32 = 10.0;
pub const ITEM_GAP: f32 = 6.0;
pub const MONOGRAM: f32 = 18.0;
pub const MONOGRAM_RADIUS: f32 = 4.0;
pub const SMALL_TEXT: f32 = 11.0;
pub const MONOGRAM_PAD: f32 = 4.0;
pub const MONOGRAM_GAP: f32 = 8.0;
pub const CAP_HEIGHT: f32 = 20.0;
pub const CAP_RADIUS: f32 = 4.0;
pub const CAP_PAD: f32 = 6.0;
pub const DOT: f32 = 6.0;
pub const NOTICE_FIELD_MIN: f32 = 120.0;
pub const TAB_NAME_MAX: f32 = 160.0;
pub const SESSION_NAME_MAX: f32 = 180.0;
pub const PANEL_RADIUS: f32 = 10.0;
pub const PANEL_PAD_Y: f32 = 20.0;
pub const PANEL_PAD_X: f32 = 24.0;
pub const PANEL_WIDTH: f32 = 720.0;
pub const PANEL_TOP: f32 = 200.0;
pub const PANEL_TALL: f32 = 600.0;
pub const FIELD_HEIGHT: f32 = 32.0;
pub const FIELD_RADIUS: f32 = 6.0;
pub const FIELD_GAP: f32 = 12.0;
pub const CARET_GAP: f32 = 4.0;
pub const ROW_HEIGHT: f32 = 32.0;
pub const ROW_RADIUS: f32 = 6.0;
pub const ROW_PAD: f32 = 10.0;
pub const ROW_GAP: f32 = 4.0;
pub const FOOTER_GAP: f32 = 16.0;
pub const FOOTER_RULE_GAP: f32 = 12.0;
pub const BACKDROP: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 102,
};
pub const ELLIPSIS: &str = "…";
pub const ENTER_KEY: &str = "↵";
pub const MOVE_KEY: &str = "↑↓";
pub const ESCAPE_KEY: &str = "esc";
pub const DOCK_WIDTH: f32 = 320.0;
pub const DOCK_INSET: f32 = 8.0;
pub const NAV_WIDTH: f32 = 232.0;
pub const NAV_NARROW: f32 = 160.0;
pub const NAV_BREAKPOINT: f32 = 1200.0;
pub const NAV_PAD_X: f32 = 16.0;
pub const SECTION_HEIGHT: f32 = 28.0;
pub const CONTENT_INSET: f32 = 40.0;
pub const CONTENT_WIDTH: f32 = 760.0;
pub const HEADER_GAP: f32 = 24.0;
pub const GROUP_GAP: f32 = 24.0;
pub const HEADING_GAP: f32 = 8.0;
pub const ENTRY_HEIGHT: f32 = 56.0;
pub const ENTRY_PAD: f32 = 16.0;
pub const ENTRY_TOP: f32 = 10.0;
pub const LINE_BOX: f32 = 18.0;
pub const HIGHLIGHT_INSET: f32 = 4.0;
pub const CONTROL_GAP: f32 = 12.0;
pub const CONTROL_MAX: f32 = 240.0;
pub const TOGGLE_WIDTH: f32 = 28.0;
pub const TOGGLE_HEIGHT: f32 = 16.0;
pub const TOGGLE_RADIUS: f32 = 8.0;
pub const KNOB: f32 = 12.0;
pub const KNOB_INSET: f32 = 2.0;
pub const STEP_ZONE: f32 = 20.0;
pub const CHIP_MIN: f32 = 104.0;
pub const SWATCH: f32 = 14.0;
pub const SWATCH_RADIUS: f32 = 4.0;
pub const ERROR_HEIGHT: f32 = 20.0;
pub const CHORD_COLUMN: f32 = 72.0;
pub const STEP_DOWN: &str = "‹";
pub const STEP_UP: &str = "›";
pub const CHOICE_MARK: &str = "▾";
pub const STEP_KEY: &str = "←→";
pub const SPACE_KEY: &str = "space";
pub const DELETE_KEY: &str = "del";
pub const SEARCH_KEY: &str = "/";
pub const CRUMB_GAP: &str = " / ";

pub fn px(value: f32, scale: f32) -> f32 {
    (value * scale).round()
}
