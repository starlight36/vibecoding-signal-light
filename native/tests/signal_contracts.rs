mod common;

use signal_light_native::signals;

#[test]
fn public_signal_names_match_contract() {
    let names = signals::definitions().keys().cloned().collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "attention",
            "blocked",
            "done",
            "idle",
            "off",
            "permission",
            "session_done",
            "session_end",
            "session_start",
            "thinking",
            "tool_done",
            "working",
        ]
    );
}

#[test]
fn working_and_thinking_share_the_same_work_cycle() {
    let working = signals::signal("working").unwrap();
    let thinking = signals::signal("thinking").unwrap();
    assert_eq!(working.frames, thinking.frames);
    assert_eq!(working.frames.len(), 27);
    assert_eq!(working.frames[0].brightness_hint, Some(0.10));
    assert!(working.frames[9].yellow_on);
    assert!(working.frames[18].red_on);
}

#[test]
fn aggregate_priority_preserves_attention_and_blocking_rules() {
    assert_eq!(
        signals::aggregate_signals(["working", "attention"]),
        "attention"
    );
    assert_eq!(
        signals::aggregate_signals(["attention", "permission", "working"]),
        "permission"
    );
    assert_eq!(
        signals::aggregate_signals(["permission", "blocked"]),
        "blocked"
    );
    assert_eq!(signals::aggregate_signals(["tool_done"]), "working");
    assert_eq!(
        signals::aggregate_signals(std::iter::empty::<&str>()),
        "idle"
    );
}

#[test]
fn session_done_duration_matches_notice_budget_expectation() {
    let notice = signals::signal(signals::SESSION_END_NOTICE_SIGNAL).unwrap();
    assert_eq!(notice.duration_ms(0.05), 96);
}
