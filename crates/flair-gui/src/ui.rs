use flair_core::theme::Rgba;

pub const BAR: f32 = 34.0;
pub const LINE: f32 = 1.0;
pub const INSET: f32 = 8.0;
pub const ITEM_HEIGHT: f32 = 24.0;
pub const ITEM_PAD: f32 = 10.0;
pub const ITEM_GAP: f32 = 6.0;
pub const PROCESS_ICON_GAP: f32 = 6.0;
pub const SMALL_TEXT: f32 = 11.0;
pub const UNDERLINE: f32 = 2.0;
pub const UNDERLINE_INSET: f32 = 8.0;
pub const SELECT_BAR: f32 = 2.0;
pub const SELECT_INSET: f32 = 7.0;
pub const PALETTE_FIELD: f32 = 44.0;
pub const PALETTE_GAP: f32 = 8.0;
pub const NUMBER_COLUMN: f32 = 20.0;
pub const DEPTH_INDENT: f32 = 20.0;
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
pub const FIELD_RADIUS: f32 = 6.0;
pub const CARET_GAP: f32 = 4.0;
pub const ROW_HEIGHT: f32 = 32.0;
pub const ROW_RADIUS: f32 = 6.0;
pub const ROW_PAD: f32 = 10.0;
pub const ROW_GAP: f32 = 4.0;
pub const FOOTER_GAP: f32 = 16.0;
pub const FOOTER_RULE_GAP: f32 = 12.0;
pub const STRIP_HEIGHT: f32 = 120.0;
pub const LABEL_HEIGHT: f32 = 20.0;
pub const THUMB_MAX: f32 = 200.0;
pub const THUMB_GAP: f32 = 12.0;
pub const THUMB_RADIUS: f32 = 6.0;
pub const THUMB_PAD: f32 = 3.0;
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
pub const GROUP_GAP: f32 = 24.0;
pub const HEADING_GAP: f32 = 8.0;
pub const CONTROL_GAP: f32 = 12.0;
pub const STEP_ZONE: f32 = 20.0;
pub const SWATCH: f32 = 14.0;
pub const SWATCH_RADIUS: f32 = 4.0;
pub const SEARCH_KEY: &str = "/";
pub const CRUMB_GAP: &str = " / ";
pub const PANEL_ROW: f32 = 28.0;
pub const EDGE_GRAB: f32 = 6.0;
pub const SWITCHER: f32 = 36.0;
pub const DRAG_START: f32 = 4.0;
pub const CLOSE_KEY: char = '\u{ea76}';
pub const BELL_KEY: char = '\u{eaa2}';
pub const ADD_KEY: char = '\u{ea60}';
pub const TREE_ROW: f32 = 24.0;
pub const TREE_INDENT: f32 = 16.0;
pub const CHEVRON_RIGHT: char = '\u{eab6}';
pub const CHEVRON_DOWN: char = '\u{eab4}';
pub const FILES_KEY: char = '\u{eaf0}';

pub fn px(value: f32, scale: f32) -> f32 {
    (value * scale).round()
}
