use lazyaiden_profiles::slugify;
use proptest::prelude::*;

#[test]
fn examples() {
    assert_eq!(slugify("Morning V60"), "morning-v60");
    assert_eq!(slugify("  Hello,   World!! "), "hello-world");
    assert_eq!(slugify("A/B+C"), "a-b-c");
    assert_eq!(slugify("!!!"), "profile");
    assert_eq!(slugify(""), "profile");
    assert_eq!(slugify("Café"), "caf");
}

proptest! {
    #[test]
    fn slugs_are_always_safe_file_names(title in ".{0,80}") {
        let s = slugify(&title);
        prop_assert!(!s.is_empty());
        prop_assert!(s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'));
        prop_assert!(!s.starts_with('-') && !s.ends_with('-') && !s.contains("--"));
    }

    #[test]
    fn slugify_is_idempotent(title in "[ -~]{0,60}") {
        let once = slugify(&title);
        prop_assert_eq!(slugify(&once), once);
    }
}
