use super::*;

#[test]
fn an_empty_repository_offers_to_version_a_block() {
    let mut harness = harness(Repository::default(), VersionStatus::default());

    harness
        .editor
        .snapshot("an_empty_repository_offers_to_version_a_block");
    assert!(
        harness
            .editor
            .document()
            .find_test_id("repository.adopt")
            .is_some()
    );
}
