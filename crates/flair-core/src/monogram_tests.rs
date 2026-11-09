use super::*;

#[test]
fn letters_follow_words_or_the_first_two_characters() {
    for (name, expected) in [
        ("flair", "FL"),
        ("rust-catchers", "RC"),
        ("DENIC-Configs", "DC"),
        ("DenicConfigs", "DC"),
        ("x", "X"),
        ("Über", "ÜB"),
        ("my_project.rs", "MP"),
        ("two words", "TW"),
        ("-lead", "LE"),
        ("", ""),
    ] {
        assert_eq!(monogram(name).letters, expected, "{name}");
    }
}

#[test]
fn hue_is_stable_and_from_the_palette() {
    let first = hue_index("flair");
    assert_eq!(first, hue_index("flair"));
    assert_eq!(first, 3);
    let mark = monogram("flair");
    assert_eq!(mark.hue, MONOGRAM_HUES[first]);
    assert_eq!(mark.text, blend(Rgb::default(), mark.hue, 20));
}

#[test]
fn fnv1a_matches_the_reference_vectors() {
    assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
}
