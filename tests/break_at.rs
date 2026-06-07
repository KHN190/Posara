use posara::debug::{parse_break_spec, break_matches};

#[test]
fn parse_valid_specs() {
    assert_eq!(parse_break_spec("update:12"), Some(("update".to_string(), 12)));
    assert_eq!(parse_break_spec("#99:0"), Some(("#99".to_string(), 0)));
    assert_eq!(parse_break_spec("a::b:3"), Some(("a::b".to_string(), 3)));
}

#[test]
fn parse_invalid_specs() {
    assert_eq!(parse_break_spec("update"), None);
    assert_eq!(parse_break_spec("update:abc"), None);
    assert_eq!(parse_break_spec(""), None);
}

#[test]
fn match_by_name_and_id() {
    let names = vec!["main".to_string(), "step".to_string()];
    assert!(break_matches("step", 6, 1, 6, &names));
    assert!(!break_matches("step", 6, 1, 7, &names));
    assert!(!break_matches("step", 6, 0, 6, &names));
    assert!(break_matches("#1", 6, 1, 6, &names));
    assert!(!break_matches("#2", 6, 1, 6, &names));
    assert!(!break_matches("step", 6, 1, 6, &[]));
}
