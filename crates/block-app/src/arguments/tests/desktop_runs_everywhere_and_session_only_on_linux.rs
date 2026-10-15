use super::*;

#[test]
fn desktop_runs_everywhere_and_session_only_on_linux() {
    for linux in [true, false] {
        assert_eq!(
            parse(&words("--dev-workspace --desktop"), linux),
            Ok(Arguments {
                dev_workspace: true,
                desktop: true,
                ..Arguments::default()
            }),
            "--desktop runs the desktop shell in a window (linux: {linux})"
        );
        assert!(parse(&words("--bogus"), linux).is_err());
    }

    assert_eq!(
        parse(&words("--session"), true),
        Ok(Arguments {
            desktop: true,
            session: true,
            ..Arguments::default()
        }),
        "a session is the desktop shell on the displays themselves"
    );
    assert_eq!(
        parse(&words("--install-session /opt"), true),
        Ok(Arguments {
            install_session: Some(Some("/opt".to_owned())),
            ..Arguments::default()
        })
    );

    for linux_only in ["--session", "--install-session"] {
        let refused = parse(&words(linux_only), false).expect_err("only Linux has sessions");
        assert!(
            refused.contains("--desktop"),
            "refusing {linux_only} points at --desktop: {refused}"
        );
    }
}
