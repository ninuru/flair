use flair_core::theme::{Rgb, Rgba};

const fn rgb(value: u32) -> Rgb {
    Rgb {
        r: (value >> 16) as u8,
        g: (value >> 8) as u8,
        b: value as u8,
    }
}

pub(crate) const DIM: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 163,
};
pub(crate) const SHADOW: Rgba = Rgba {
    r: 0,
    g: 0,
    b: 0,
    a: 34,
};
pub(crate) const BACKGROUND: Rgb = rgb(0x000000);
pub(crate) const FOREGROUND: Rgb = rgb(0xf5f5f5);
pub(crate) const MUTED: Rgb = rgb(0xa1a1a1);
pub(crate) const QUIET: Rgb = rgb(0x848484);
pub(crate) const ICON: Rgb = rgb(0x8b8b8b);
pub(crate) const CHEVRON: Rgb = rgb(0x717171);
pub(crate) const PRIMARY: Rgb = rgb(0x346bf1);
pub(crate) const ERROR: Rgb = rgb(0xf26d6d);
pub(crate) const PANEL_BORDER: Rgb = rgb(0x1c1c1c);
pub(crate) const HAIRLINE: Rgb = rgb(0x141414);
pub(crate) const SIDEBAR: Rgb = rgb(0x111111);
pub(crate) const SIDEBAR_BORDER: Rgb = rgb(0x1f1f1f);
pub(crate) const NAV_CURRENT: Rgb = rgb(0x1d1d1d);
pub(crate) const NAV_HOVER: Rgb = rgb(0x181818);
pub(crate) const KBD: Rgb = rgb(0x222222);
pub(crate) const CARD: Rgb = rgb(0x070707);
pub(crate) const CARD_BORDER: Rgb = rgb(0x151515);
pub(crate) const DIVIDER: Rgb = rgb(0x121212);
pub(crate) const ROW_FOCUS: Rgb = rgb(0x111111);
pub(crate) const ROW_HOVER: Rgb = rgb(0x0c0c0c);
pub(crate) const GHOST_HOVER: Rgb = rgb(0x161616);
pub(crate) const INPUT: Rgb = rgb(0x101010);
pub(crate) const INPUT_BORDER: Rgb = rgb(0x262626);
pub(crate) const SWITCH_OFF: Rgb = rgb(0x262626);
pub(crate) const SWITCH_THUMB_ON: Rgb = rgb(0x000000);
pub(crate) const CHORD: Rgb = rgb(0x1a1a1a);
pub(crate) const POPUP: Rgb = rgb(0x111111);
pub(crate) const POPUP_BORDER: Rgb = rgb(0x272727);
pub(crate) const POPUP_SELECTED: Rgb = rgb(0x232323);
pub(crate) const POPUP_HOVER: Rgb = rgb(0x1b1b1b);

pub(crate) const INSET: f32 = 40.0;
pub(crate) const INSET_SMALL: f32 = 24.0;
pub(crate) const SMALL_WIDTH: f32 = 1024.0;
pub(crate) const SMALL_HEIGHT: f32 = 720.0;
pub(crate) const PANEL_MAX_W: f32 = 1280.0;
pub(crate) const PANEL_MAX_H: f32 = 960.0;
pub(crate) const PANEL_RADIUS: f32 = 16.0;
pub(crate) const SHADOW_DROP: f32 = 12.0;
pub(crate) const SHADOW_SPREAD: f32 = 6.0;
pub(crate) const SHADOW_LAYERS: usize = 4;

pub(crate) const SIDEBAR_WIDTH: f32 = 240.0;
pub(crate) const RAIL_WIDTH: f32 = 56.0;
pub(crate) const SIDEBAR_MIN_CONTENT: f32 = 480.0;
pub(crate) const RAIL_MIN_CONTENT: f32 = 320.0;
pub(crate) const SIDEBAR_PAD: f32 = 8.0;
pub(crate) const NAV_ITEM: f32 = 32.0;
pub(crate) const NAV_ITEM_PAD: f32 = 10.0;
pub(crate) const NAV_ITEM_GAP: f32 = 4.0;
pub(crate) const NAV_ITEM_RADIUS: f32 = 8.0;
pub(crate) const NAV_GROUP_GAP: f32 = 16.0;
pub(crate) const NAV_HEADING: f32 = 16.0;
pub(crate) const NAV_HEADING_GAP: f32 = 4.0;
pub(crate) const SEARCH_RADIUS: f32 = 6.0;
pub(crate) const ICON_BOX: f32 = 16.0;
pub(crate) const ICON_GAP: f32 = 8.0;
pub(crate) const KBD_HEIGHT: f32 = 20.0;
pub(crate) const KBD_PAD: f32 = 4.0;
pub(crate) const KBD_RADIUS: f32 = 4.0;

pub(crate) const HEADER_HEIGHT: f32 = 48.0;
pub(crate) const HEADER_PAD_LEFT: f32 = 20.0;
pub(crate) const HEADER_PAD_RIGHT: f32 = 8.0;
pub(crate) const CRUMB_GAP: f32 = 12.0;
pub(crate) const CLOSE_SIZE: f32 = 28.0;
pub(crate) const GHOST_RADIUS: f32 = 8.0;

pub(crate) const CONTAINER_MAX: f32 = 896.0;
pub(crate) const CONTENT_PAD_X: f32 = 24.0;
pub(crate) const CONTENT_PAD_TOP: f32 = 24.0;
pub(crate) const CONTENT_PAD_BOTTOM: f32 = 24.0;
pub(crate) const CONTENT_MIN: f32 = 240.0;
pub(crate) const SECTION_GAP: f32 = 32.0;
pub(crate) const HEADING_HEIGHT: f32 = 24.0;
pub(crate) const HEADING_PAD: f32 = 16.0;
pub(crate) const HEADING_GAP: f32 = 10.0;
pub(crate) const CARD_RADIUS: f32 = 12.0;

pub(crate) const ROW_PAD_X: f32 = 16.0;
pub(crate) const ROW_PAD_Y: f32 = 12.0;
pub(crate) const ROW_TALL: f32 = 66.0;
pub(crate) const ROW_SHORT: f32 = 44.0;
pub(crate) const TITLE_LINE: f32 = 20.0;
pub(crate) const DESCRIPTION_GAP: f32 = 4.0;
pub(crate) const DESCRIPTION_LINE: f32 = 18.0;
pub(crate) const ERROR_LINE: f32 = 18.0;
pub(crate) const CONTROL_GAP: f32 = 32.0;
pub(crate) const FOCUS_INSET: f32 = 4.0;
pub(crate) const FOCUS_RADIUS: f32 = 8.0;
pub(crate) const UNDO_SIZE: f32 = 20.0;
pub(crate) const UNDO_GAP: f32 = 6.0;
pub(crate) const UNDO_RADIUS: f32 = 6.0;

pub(crate) const SWITCH_WIDTH: f32 = 30.0;
pub(crate) const SWITCH_HEIGHT: f32 = 18.0;
pub(crate) const SWITCH_PAD: f32 = 2.0;
pub(crate) const THUMB: f32 = 14.0;
pub(crate) const RING: f32 = 2.0;
pub(crate) const SELECT_WIDTH: f32 = 176.0;
pub(crate) const TEXT_WIDTH: f32 = 240.0;
pub(crate) const SELECT_HEIGHT: f32 = 28.0;
pub(crate) const SELECT_PAD: f32 = 9.0;
pub(crate) const SELECT_RADIUS: f32 = 8.0;
pub(crate) const STEP_ZONE: f32 = 22.0;
pub(crate) const SWATCH: f32 = 14.0;
pub(crate) const SWATCH_RADIUS: f32 = 4.0;
pub(crate) const SWATCH_GAP: f32 = 8.0;
pub(crate) const CHORD_PAD: f32 = 6.0;
pub(crate) const CHORD_GAP: f32 = 12.0;

pub(crate) const POPUP_GAP: f32 = 4.0;
pub(crate) const POPUP_PAD: f32 = 4.0;
pub(crate) const POPUP_ROW: f32 = 28.0;
pub(crate) const POPUP_ROW_PAD: f32 = 8.0;
pub(crate) const POPUP_ROW_RADIUS: f32 = 4.0;
pub(crate) const POPUP_ROWS: usize = 10;

pub(crate) const PALETTE: Rgb = rgb(0x0a0a0a);
pub(crate) const PALETTE_SHINE: Rgb = rgb(0x141414);
pub(crate) const PALETTE_HIGHLIGHT: Rgb = rgb(0x1f1f1f);
pub(crate) const PALETTE_CURRENT: Rgb = rgb(0x181818);
pub(crate) const PALETTE_FOOTER: Rgb = rgb(0x101010);
pub(crate) const PALETTE_FOOTER_BORDER: Rgb = rgb(0x1a1a1a);
pub(crate) const PALETTE_KEYCAP: Rgb = rgb(0x1e1e1e);
pub(crate) const MONOGRAM: Rgb = rgb(0x262626);
pub(crate) const PALETTE_WIDTH: f32 = 576.0;
pub(crate) const PALETTE_DOCK_WIDTH: f32 = 360.0;
pub(crate) const PALETTE_MAX_H: f32 = 420.0;
pub(crate) const PALETTE_SIDE: f32 = 16.0;
pub(crate) const PALETTE_TOP: f32 = 0.1;
pub(crate) const PALETTE_BAR_GAP: f32 = 8.0;
pub(crate) const FIELD_PAD_X: f32 = 8.0;
pub(crate) const FIELD_PAD_Y: f32 = 6.0;
pub(crate) const FIELD_ROW: f32 = 34.0;
pub(crate) const FIELD_ICON_X: f32 = 4.0;
pub(crate) const FIELD_TEXT_X: f32 = 32.0;
pub(crate) const LIST_PAD: f32 = 8.0;
pub(crate) const GROUP_LABEL: f32 = 28.0;
pub(crate) const ITEM_ROW: f32 = 32.0;
pub(crate) const ITEM_PAD_X: f32 = 8.0;
pub(crate) const ITEM_GAP: f32 = 8.0;
pub(crate) const ITEM_RADIUS: f32 = 4.0;
pub(crate) const ITEM_INDENT: f32 = 16.0;
pub(crate) const NAME_MAX: f32 = 220.0;
pub(crate) const EMPTY_HEIGHT: f32 = 100.0;
pub(crate) const FOOTER_HEIGHT: f32 = 40.0;
pub(crate) const FOOTER_PAD_X: f32 = 16.0;
pub(crate) const HINT_GAP: f32 = 12.0;
pub(crate) const HINT_INNER: f32 = 4.0;
pub(crate) const STRIP_PAD: f32 = 12.0;

pub(crate) const BAR_PAD: f32 = 12.0;
pub(crate) const BAR_TINT: u8 = 5;
pub(crate) const CRUMB_SPACE: f32 = 8.0;
pub(crate) const TAB_HEIGHT: f32 = 24.0;
pub(crate) const TAB_RADIUS: f32 = 6.0;
pub(crate) const TAB_PAD: f32 = 8.0;
pub(crate) const TAB_GAP: f32 = 4.0;
pub(crate) const TAB_ACTIVE_TINT: u8 = 8;
pub(crate) const TAB_HOVER_TINT: u8 = 4;
pub(crate) const BUTTON_SIZE: f32 = 28.0;
pub(crate) const BUTTON_RADIUS: f32 = 8.0;
pub(crate) const BUTTON_GAP: f32 = 4.0;
pub(crate) const BUTTON_HOVER_TINT: u8 = 6;
pub(crate) const BADGE_HEIGHT: f32 = 16.0;
pub(crate) const BADGE_RADIUS: f32 = 4.0;
pub(crate) const BADGE_PAD: f32 = 4.0;
pub(crate) const BADGE_TINT: u8 = 16;
pub(crate) const BADGE_NEUTRAL_TINT: u8 = 10;

pub(crate) const SEARCH_ICON: char = '\u{ea6d}';
pub(crate) const EDIT_ICON: char = '\u{ea73}';
pub(crate) const CLOSE_ICON: char = '\u{ea76}';
pub(crate) const CHEVRON_ICON: char = '\u{eab4}';
pub(crate) const UNDO_ICON: char = '\u{eae2}';
pub(crate) const MINUS_ICON: char = '\u{eb3b}';
pub(crate) const PLUS_ICON: char = '\u{ea60}';
pub(crate) const APPEARANCE_ICON: char = '\u{eb5c}';
pub(crate) const FONT_ICON: char = '\u{eb69}';
pub(crate) const WINDOW_ICON: char = '\u{eb7f}';
pub(crate) const PANELS_ICON: char = '\u{ebf3}';
pub(crate) const TERMINAL_ICON: char = '\u{ea85}';
pub(crate) const PROMPT_ICON: char = '\u{eb9b}';
pub(crate) const KEYS_ICON: char = '\u{ea65}';
pub(crate) const SEARCH_KEY: &str = "/";
pub(crate) const CRUMB_SLASH: &str = "/";
