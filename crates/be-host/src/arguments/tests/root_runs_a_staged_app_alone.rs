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
    assert_eq!(
        parse(&words("--root"), None, LINUX),
        Ok(Arguments {
            root: Some("be3.root".to_owned()),
            ..Arguments::default()
        }),
        "--root alone runs be-root"
    );
    assert!(
        parse(&words("--root="), None, LINUX).is_err(),
        "--root= names no app"
    );
    assert_eq!(
        parse(&words("--root --desktop"), None, LINUX),
        Ok(Arguments {
            root: Some("be3.root".to_owned()),
            desktop: true,
            ..Arguments::default()
        }),
        "the root shows the desktop shell as its session's editor"
    );
    assert_eq!(
        parse(&words("--dev-workspace --root"), None, LINUX),
        Ok(Arguments {
            root: Some("be3.root".to_owned()),
            dev_workspace: true,
            ..Arguments::default()
        }),
        "the dev workspace is the host's block stack's, which a root shows"
    );
    assert!(
        parse(&words("--root=be3.root-testing"), Some(DRM), true).is_err(),
        "a login session needs the lock screen, which a root does not have yet"
    );
}
