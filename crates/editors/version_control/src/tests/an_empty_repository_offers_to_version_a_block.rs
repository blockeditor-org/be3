use super::*;

#[test]
fn an_empty_repository_offers_to_version_a_block() {
    let mut editor = repository(Repository::default(), VersionStatus::default());

    assert!(editor.shown("repository.adopt"));
    editor.snapshot("an_empty_repository_offers_to_version_a_block");
}
