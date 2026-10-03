use octoscode_module::screens::sidebar::bounded_width;

#[test]
fn dragging_keeps_navigation_and_the_conversation_usable() {
    assert_eq!(bounded_width(360.0, 1280.0), 360.0);
    assert_eq!(bounded_width(-100.0, 1280.0), 280.0);
    assert_eq!(bounded_width(900.0, 1280.0), 520.0);
    assert_eq!(bounded_width(500.0, 800.0), 380.0);
    assert_eq!(bounded_width(f64::NAN, 1280.0), 280.0);
}
