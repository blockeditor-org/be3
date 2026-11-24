use super::*;

#[test]
fn a_start_menu_shortcut_becomes_a_program() {
    let program = Program {
        id: "Accessories/Paint".to_owned(),
        name: "Paint".to_owned(),
        folder: Some("Accessories".to_owned()),
        path: "C:\\Start Menu\\Accessories\\Paint.lnk".into(),
        shortcut: Shortcut {
            target: Some("C:\\Windows\\paint.exe".into()),
            arguments: String::new(),
            description: "Draw pictures".to_owned(),
            app_user_model_id: Some("Microsoft.Paint".to_owned()),
        },
    };
    let host = host_program(&program, None);
    assert_eq!(host.id, "Accessories/Paint");
    assert_eq!(host.name, "Paint");
    assert_eq!(host.comment, "Draw pictures");
    assert_eq!(host.keywords, vec!["Accessories".to_owned()]);
    assert_eq!(
        host.wm_class.as_deref(),
        Some("Microsoft.Paint"),
        "its windows carry its AppUserModelID"
    );

    // A shortcut with no AppUserModelID: its windows carry its target's path, which a shell
    // matches a window from last time by, to start it again.
    let plain = Program {
        shortcut: Shortcut {
            app_user_model_id: None,
            ..program.shortcut.clone()
        },
        ..program
    };
    assert_eq!(
        host_program(&plain, None).wm_class.as_deref(),
        Some("C:\\Windows\\paint.exe")
    );
}
