use super::*;

#[test]
fn desktop_runs_everywhere_and_on_the_displays_only_on_linux() {
    for linux in [true, false] {
        assert_eq!(
            parse(&words("--dev-workspace --desktop"), None, linux),
            Ok(Arguments {
                dev_workspace: true,
                desktop: true,
                ..Arguments::default()
            }),
            "--desktop runs the desktop shell in a window (linux: {linux})"
        );
        assert!(parse(&words("--bogus"), None, linux).is_err());
        assert!(parse(&words("--session"), None, linux).is_err());
    }

    assert_eq!(
        parse(&words("--desktop"), Some("drm"), true),
        Ok(Arguments {
            desktop: true,
            drm: true,
            ..Arguments::default()
        }),
        "a login session is the desktop shell on the displays themselves"
    );
    assert_eq!(
        parse(&words("--desktop"), Some("winit"), true),
        Ok(Arguments {
            desktop: true,
            ..Arguments::default()
        }),
        "beui checks every other platform name"
    );
    assert_eq!(
        parse(&words(""), Some("drm"), true),
        Ok(Arguments {
            drm: true,
            ..Arguments::default()
        }),
        "the workspace shell may run on the displays too"
    );
    assert_eq!(
        parse(&words("--install-login-session /opt"), None, true),
        Ok(Arguments {
            install_login_session: Some(Some("/opt".to_owned())),
            ..Arguments::default()
        })
    );

    for refused in [
        parse(&words("--desktop"), Some("drm"), false),
        parse(&words("--install-login-session"), None, false),
    ] {
        let refused = refused.expect_err("only Linux has login sessions");
        assert!(
            refused.contains("--desktop runs the desktop shell in a window"),
            "refusing what only Linux has points at --desktop: {refused}"
        );
    }
}
