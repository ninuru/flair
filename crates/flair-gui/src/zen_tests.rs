use super::*;

#[test]
fn toggle_flips_zen() {
    let zen = Zen::default().toggled();
    assert!(zen.on());
    assert!(!zen.toggled().on());
}

#[test]
fn entering_fullscreen_turns_zen_on_and_leaving_turns_it_off() {
    let zen = Zen::default().fullscreen(true);
    assert!(zen.on());
    assert!(!zen.fullscreen(false).on());
}

#[test]
fn leaving_fullscreen_keeps_zen_that_was_on_before() {
    let zen = Zen::default().toggled().fullscreen(true).fullscreen(false);
    assert!(zen.on());
}

#[test]
fn toggle_in_fullscreen_holds_until_fullscreen_ends() {
    let zen = Zen::default().fullscreen(true).toggled();
    assert!(!zen.on());
    assert!(!zen.fullscreen(true).on());
    assert!(!zen.fullscreen(false).on());
}

#[test]
fn repeated_fullscreen_reports_do_not_change_zen() {
    let zen = Zen::default().fullscreen(true).toggled().fullscreen(true);
    assert!(!zen.on());
    assert_eq!(Zen::default().fullscreen(false), Zen::default());
}
