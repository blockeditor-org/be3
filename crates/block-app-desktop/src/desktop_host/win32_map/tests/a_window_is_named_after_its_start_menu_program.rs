use super::*;

fn program(id: &str, name: &str, target: &str, app_user_model_id: Option<&str>) -> Program {
    Program {
        id: id.to_owned(),
        name: name.to_owned(),
        shortcut: Shortcut {
            target: Some(target.into()),
            app_user_model_id: app_user_model_id.map(str::to_owned),
            ..Shortcut::default()
        },
        ..Program::default()
    }
}

fn named(names: &ProgramNames, app_id: &str, origin: Origin) -> String {
    let entry = Entry {
        app_id: app_id.to_owned(),
        origin,
        ..window(1, Rect::new(0, 0, 100, 100))
    };
    names.name(&entry)
}

#[test]
fn a_window_is_named_after_its_start_menu_program() {
    let names = ProgramNames::new(&[
        program("Accessories/Paint", "Paint", "C:\\Windows\\System32\\mspaint.exe", None),
        program(
            "Calculator",
            "Calculator",
            "C:\\Windows\\explorer.exe",
            Some("Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"),
        ),
        program("Paint Copy", "Paint again", "C:\\Windows\\System32\\mspaint.exe", None),
    ]);
    let none = Origin::default;
    assert_eq!(
        named(&names, "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App", none()),
        "Calculator",
        "by its AppUserModelID"
    );
    assert_eq!(
        named(&names, "c:/windows/system32/MSPAINT.EXE", none()),
        "Paint",
        "by its executable, ignoring case and slashes; the first shortcut to it wins"
    );
    assert_eq!(
        named(
            &names,
            "Some.Unlisted.Id",
            Origin {
                command: vec!["C:\\Windows\\System32\\mspaint.exe".to_owned()],
                ..Origin::default()
            }
        ),
        "Paint",
        "by the command the desktop ran"
    );
    assert_eq!(
        named(
            &names,
            "C:\\Windows\\System32\\mspaint.exe",
            Origin {
                program: Some("Paint Copy".to_owned()),
                ..Origin::default()
            }
        ),
        "Paint again",
        "the program the desktop launched comes first"
    );
    assert_eq!(named(&names, "C:\\Tools\\unknown.exe", none()), "");
    assert_eq!(named(&ProgramNames::default(), "", none()), "");
}
