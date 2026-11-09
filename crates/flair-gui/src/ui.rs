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
pub const MONOGRAM_TEXT: f32 = 10.0;
pub const MONOGRAM_PAD: f32 = 4.0;
pub const MONOGRAM_GAP: f32 = 8.0;
pub const CAP_HEIGHT: f32 = 20.0;
pub const CAP_RADIUS: f32 = 4.0;
pub const CAP_PAD: f32 = 6.0;
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

pub fn px(value: f32, scale: f32) -> f32 {
    (value * scale).round()
}
