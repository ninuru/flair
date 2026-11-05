use super::*;

#[test]
fn shelf_packing_wraps_and_clear_resets() {
    let mut atlas = Atlas::new(8);
    assert_eq!(
        atlas.insert(5, 3),
        Some(AtlasRect {
            x: 0,
            y: 0,
            w: 5,
            h: 3
        })
    );
    assert_eq!(
        atlas.insert(4, 2),
        Some(AtlasRect {
            x: 0,
            y: 3,
            w: 4,
            h: 2
        })
    );
    assert_eq!(atlas.insert(8, 4), None);
    atlas.clear();
    assert_eq!(
        atlas.insert(8, 8),
        Some(AtlasRect {
            x: 0,
            y: 0,
            w: 8,
            h: 8
        })
    );
    assert_eq!(atlas.insert(1, 1), None);
}

#[test]
fn oversized_or_empty_glyph_does_not_consume_space() {
    let mut atlas = Atlas::new(4);
    assert_eq!(atlas.insert(5, 1), None);
    assert_eq!(atlas.insert(0, 1), None);
    assert_eq!(
        atlas.insert(4, 4),
        Some(AtlasRect {
            x: 0,
            y: 0,
            w: 4,
            h: 4
        })
    );
}

#[test]
fn cache_rasterizes_once_and_recovers_after_clear() {
    let mut cache = GlyphCache::new(8);
    let key = GlyphKey {
        c: 'x',
        style: Style::Regular,
    };
    let raster = || {
        Some(RasterGlyph {
            width: 2,
            height: 3,
            left: 0,
            top: 2,
            colored: false,
            pixels: vec![255; 24],
        })
    };
    assert!(cache.get_or_insert(key, raster).is_some());
    assert!(
        cache
            .get_or_insert(key, || panic!("cached glyph rasterized again"))
            .is_some()
    );
    cache.clear();
    assert!(cache.get_or_insert(key, raster).is_some());
}
