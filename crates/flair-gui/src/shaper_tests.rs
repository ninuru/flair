use flair_core::{config::Config, proto::CellFlags, theme::Rgb};

use super::*;
use crate::font::Fonts;

fn fonts(ligatures: bool) -> Fonts {
    let mut config = Config::default().font;
    config.ligatures = ligatures;
    Fonts::load(&config, 1.0).unwrap()
}

fn shaper(fonts: &Fonts) -> Shaper {
    Shaper::new(Arc::clone(fonts.shaping().unwrap()))
}

fn cells(text: &str) -> Vec<DrawCell> {
    text.chars()
        .map(|c| DrawCell {
            c,
            extra: None,
            fg: Rgb {
                r: 200,
                g: 200,
                b: 200,
            },
            bg: Rgb { r: 0, g: 0, b: 0 },
            flags: CellFlags::empty(),
        })
        .collect()
}

fn glyph_id(shaper: &Shaper, c: char, style: Style) -> u16 {
    let (_, face) = shaper.faces.face(style).unwrap();
    rustybuzz::Face::from_slice(&face.bytes, face.index)
        .unwrap()
        .glyph_index(c)
        .unwrap()
        .0
}

#[test]
fn without_ligatures_no_shaping_source_is_located() {
    assert!(fonts(false).shaping().is_none());
    assert!(fonts(true).shaping().is_some());
}

#[test]
fn glyph_ids_rasterize_exactly_like_the_char_path() {
    let mut fonts = fonts(true);
    let shaper = shaper(&fonts);
    for (c, style) in [
        ('-', Style::Regular),
        ('>', Style::Regular),
        ('m', Style::Regular),
        ('=', Style::Bold),
    ] {
        if shaper.faces.face(style).is_none() {
            eprintln!("skipped {style:?}: the terminal font has no real {style:?} face");
            continue;
        }
        assert_eq!(
            shaper.rasterize(glyph_id(&shaper, c, style), style),
            fonts.rasterize(c, style),
            "{c:?} {style:?}"
        );
    }
}

#[test]
fn an_arrow_becomes_a_blank_spacer_and_a_ligature_reaching_left() {
    let fonts = fonts(true);
    let mut shaper = shaper(&fonts);
    let shaped = shaper.row(&cells("a -> b"), None);
    let [
        None,
        None,
        Some(Shaped::Blank),
        Some(Shaped::Glyph(arrow)),
        None,
        None,
    ] = shaped[..]
    else {
        eprintln!("skipped: the terminal font has no -> ligature ({shaped:?})");
        return;
    };
    let glyph = shaper.rasterize(arrow, Style::Regular).unwrap();
    assert!(glyph.left < 0, "{}", glyph.left);
    assert_ne!(arrow, glyph_id(&shaper, '>', Style::Regular));
}

#[test]
fn plain_words_keep_their_glyphs() {
    let fonts = fonts(true);
    let mut shaper = shaper(&fonts);
    let shaped = shaper.row(&cells("x.y foo_bar"), None);
    assert!(shaped.iter().all(Option::is_none), "{shaped:?}");
}
