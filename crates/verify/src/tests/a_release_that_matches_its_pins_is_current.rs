use super::super::bootstrap::{Outcome, check};

#[test]
fn a_release_that_matches_its_pins_is_current() {
    let directory = std::env::temp_dir().join(format!("verify-unpinned-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("unpinned"), "").unwrap();
    assert!(matches!(
        check(Some(&directory), None, true),
        Ok(Outcome::Current)
    ));
    std::fs::write(directory.join("unpinned"), "bootstrap_sha256_x=abc\n").unwrap();
    assert!(matches!(
        check(Some(&directory), None, true),
        Ok(Outcome::Stale)
    ));
    std::fs::remove_dir_all(directory).unwrap();
}
