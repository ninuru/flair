use super::*;
use crate::config::Config;

fn theme() -> Theme {
    Config::default().theme()
}

#[test]
fn rgb_accepts_six_digit_hex() {
    assert_eq!(
        Rgb::from_hex("#19aAd8"),
        Ok(Rgb {
            r: 0x19,
            g: 0xaa,
            b: 0xd8
        })
    );
}

#[test]
fn rgba_accepts_six_and_eight_digit_hex() {
    assert_eq!(
        Rgba::from_hex("#19aad840"),
        Ok(Rgba {
            r: 0x19,
            g: 0xaa,
            b: 0xd8,
            a: 0x40
        })
    );
    assert_eq!(
        Rgba::from_hex("#19aad8"),
        Ok(Rgba {
            r: 0x19,
            g: 0xaa,
            b: 0xd8,
            a: 0xff
        })
    );
}

#[test]
fn malformed_hex_is_rejected() {
    for value in ["#abc", "19aad8", "#19aad", "#19aad8zz", "#gggggg", "", "#"] {
        assert!(Rgb::from_hex(value).is_err(), "{value}");
        assert!(Rgba::from_hex(value).is_err(), "{value}");
    }
    assert!(Rgb::from_hex("#19aad840").is_err());
}

#[test]
fn resolve_named_colors() {
    let theme = theme();
    assert_eq!(theme.resolve(Color::Foreground), theme.foreground);
    assert_eq!(theme.resolve(Color::Background), theme.background);
    assert_eq!(theme.resolve(Color::Cursor), theme.cursor);
}

#[test]
fn resolve_ansi_indices() {
    let theme = theme();
    for i in 0..8u8 {
        assert_eq!(
            theme.resolve(Color::Indexed(i)),
            theme.normal[usize::from(i)]
        );
        assert_eq!(
            theme.resolve(Color::Indexed(i + 8)),
            theme.bright[usize::from(i)]
        );
    }
}

#[test]
fn resolve_color_cube() {
    let theme = theme();
    assert_eq!(theme.indexed(16), Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(theme.indexed(17), Rgb { r: 0, g: 0, b: 95 });
    assert_eq!(theme.indexed(196), Rgb { r: 255, g: 0, b: 0 });
    assert_eq!(
        theme.resolve(Color::Indexed(231)),
        Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
}

#[test]
fn resolve_greyscale_ramp() {
    let theme = theme();
    assert_eq!(theme.indexed(232), Rgb { r: 8, g: 8, b: 8 });
    assert_eq!(
        theme.indexed(255),
        Rgb {
            r: 238,
            g: 238,
            b: 238
        }
    );
}

#[test]
fn resolve_truecolor() {
    assert_eq!(
        theme().resolve(Color::Rgb(1, 2, 3)),
        Rgb { r: 1, g: 2, b: 3 }
    );
}
