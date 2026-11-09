use super::*;
use crate::config::{ColorsConfig, Config};

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

#[test]
fn blend_hits_both_endpoints_and_rounds_half_up() {
    let under = Rgb {
        r: 0,
        g: 10,
        b: 255,
    };
    let over = Rgb {
        r: 255,
        g: 20,
        b: 0,
    };
    assert_eq!(blend(under, over, 0), under);
    assert_eq!(blend(under, over, 100), over);
    assert_eq!(
        blend(under, over, 50),
        Rgb {
            r: 128,
            g: 15,
            b: 128
        }
    );
    assert_eq!(
        blend(Rgb { r: 0, g: 0, b: 0 }, Rgb { r: 1, g: 3, b: 9 }, 50),
        Rgb { r: 1, g: 2, b: 5 }
    );
}

#[test]
fn flair_dark_surfaces_are_exact() {
    let theme = theme();
    let surfaces = theme.surfaces();
    let gray = |v| Rgb { r: v, g: v, b: v };
    assert_eq!(surfaces.surface1, gray(0x0d));
    assert_eq!(surfaces.surface2, gray(0x16));
    assert_eq!(surfaces.surface3, gray(0x1e));
    assert_eq!(surfaces.outline, gray(0x24));
    assert_eq!(surfaces.outline_strong, gray(0x2b));
    assert_eq!(surfaces.faint, gray(0x61));
    assert_eq!(surfaces.muted, gray(0x95));
    assert_eq!(surfaces.bright, gray(0xb5));
    assert_eq!(
        surfaces.accent_soft,
        blend(theme.background, theme.accent, 25)
    );
    assert_eq!(surfaces.running, theme.normal[2]);
    assert_eq!(surfaces.attention, theme.normal[3]);
}

#[test]
fn builtin_surfaces_are_pairwise_distinct() {
    for (name, _) in BUILTIN_THEMES {
        let theme = builtin(name);
        let s = theme.surfaces();
        let tokens = [
            theme.background,
            s.surface1,
            s.surface2,
            s.surface3,
            s.outline,
            s.outline_strong,
            s.faint,
            s.muted,
            s.bright,
            theme.foreground,
        ];
        for (i, a) in tokens.iter().enumerate() {
            for b in &tokens[i + 1..] {
                assert_ne!(a, b, "{name}");
            }
        }
    }
}

fn luminance(color: Rgb) -> f64 {
    let channel = |v: u8| {
        let c = f64::from(v) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn muted_text_on_surface1_meets_the_contrast_floor() {
    for (name, _) in BUILTIN_THEMES {
        let surfaces = builtin(name).surfaces();
        let ratio = contrast(surfaces.muted, surfaces.surface1);
        assert!(ratio >= 4.5, "{name}: {ratio}");
    }
}

fn hex(value: &str) -> Rgb {
    Rgb::from_hex(value).unwrap()
}

fn palette(values: &str) -> [Rgb; 8] {
    let colors: Vec<Rgb> = values.split(' ').map(hex).collect();
    colors.try_into().unwrap()
}

fn builtin(name: &str) -> Theme {
    let text = BUILTIN_THEMES
        .iter()
        .find(|(builtin, _)| *builtin == name)
        .map(|(_, text)| *text)
        .unwrap();
    ColorsConfig::from_toml(text).unwrap().to_theme()
}

fn selection(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 0x40,
    }
}

#[test]
fn builtin_names_are_sorted_and_complete() {
    let names: Vec<&str> = BUILTIN_THEMES.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["catppuccin-mocha", "flair-dark", "gruvbox-dark"]);
}

#[test]
fn flair_dark_builtin_equals_default_theme() {
    assert_eq!(builtin("flair-dark"), theme());
    assert_eq!(theme().cursor, hex("#d4d4d4"));
    assert_eq!(theme().selection, selection(hex("#19aad8")));
}

#[test]
fn catppuccin_mocha_matches_the_table() {
    let theme = builtin("catppuccin-mocha");
    assert_eq!(theme.background, hex("#1e1e2e"));
    assert_eq!(theme.foreground, hex("#cdd6f4"));
    assert_eq!(theme.accent, hex("#89b4fa"));
    assert_eq!(theme.cursor, hex("#f5e0dc"));
    assert_eq!(theme.selection, selection(hex("#89b4fa")));
    assert_eq!(
        theme.normal,
        palette("#45475a #f38ba8 #a6e3a1 #f9e2af #89b4fa #f5c2e7 #94e2d5 #bac2de")
    );
    assert_eq!(
        theme.bright,
        palette("#585b70 #f38ba8 #a6e3a1 #f9e2af #89b4fa #f5c2e7 #94e2d5 #a6adc8")
    );
}

#[test]
fn gruvbox_dark_matches_the_table() {
    let theme = builtin("gruvbox-dark");
    assert_eq!(theme.background, hex("#282828"));
    assert_eq!(theme.foreground, hex("#ebdbb2"));
    assert_eq!(theme.accent, hex("#fabd2f"));
    assert_eq!(theme.cursor, hex("#ebdbb2"));
    assert_eq!(theme.selection, selection(hex("#fabd2f")));
    assert_eq!(
        theme.normal,
        palette("#282828 #cc241d #98971a #d79921 #458588 #b16286 #689d6a #a89984")
    );
    assert_eq!(
        theme.bright,
        palette("#928374 #fb4934 #b8bb26 #fabd2f #83a598 #d3869b #8ec07c #ebdbb2")
    );
}

#[test]
fn builtin_themes_resolve_by_name_without_a_dir() {
    for (name, _) in BUILTIN_THEMES {
        assert_eq!(Config::default().with_theme(name).theme(), builtin(name));
    }
    assert_eq!(Config::default().with_theme("nope").theme(), theme());
}
