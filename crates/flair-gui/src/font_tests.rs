use super::*;

#[test]
fn prompt_icons_are_rasterized_larger_than_terminal_cell_icons() {
    let mut fonts = Fonts::load(&flair_core::config::Config::default().font, 1.0).unwrap();
    let key = crossfont::GlyphKey {
        character: '\u{f308}',
        font_key: fonts.faces[0],
        size: fonts.size,
    };
    if let Err(crossfont::Error::MissingGlyph(_)) = fonts.rasterizer.get_glyph(key) {
        eprintln!("skipping prompt icon comparison: the terminal font has no Nerd Font glyphs");
        return;
    }
    let Some(cell) = fonts.rasterize('\u{f308}', Style::Regular) else {
        eprintln!("skipping prompt icon comparison: Nerd Font glyph unavailable");
        return;
    };
    let Some(icon) = fonts.rasterize_prompt_icon('\u{f308}') else {
        eprintln!("skipping prompt icon comparison: sized Nerd Font glyph unavailable");
        return;
    };
    assert!(icon.height > cell.height && icon.width > cell.width);
    assert!(icon.height as f32 <= fonts.metrics().height);
}
