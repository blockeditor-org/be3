use super::*;

#[test]
fn root_runs_a_staged_app_alone() {
    assert_eq!(
        parse(&words("--root=be3.root-testing"), None, LINUX),
        Ok(Arguments {
            root: Some("be3.root-testing".to_owned()),
            ..Arguments::default()
        }),
        "--root=APP names the staged app whose entry point runs as the root"
    );
    assert!(
        parse(&words("--root="), None, LINUX).is_err(),
        "--root= names no app"
    );
    assert!(
        parse(&words("--root=be3.root-testing --desktop"), None, LINUX).is_err(),
        "the desktop is part of be-host's own UI, which a root replaces"
    );
    assert!(
        parse(&words("--dev-workspace --root=be3.root-testing"), None, LINUX).is_err(),
        "so is the dev workspace"
    );
    assert!(
        parse(&words("--root=be3.root-testing"), Some(DRM), true).is_err(),
        "a login session needs the lock screen, which a root does not have yet"
    );
}
