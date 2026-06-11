use signal_light_native::drivers::mcp2221::logical_to_physical;

#[test]
fn active_low_mapping_turns_logical_on_into_low_output() {
    assert!(!logical_to_physical(true, true));
    assert!(logical_to_physical(true, false));
}

#[test]
fn active_high_mapping_preserves_logical_output_levels() {
    assert!(logical_to_physical(false, true));
    assert!(!logical_to_physical(false, false));
}
