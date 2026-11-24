use super::*;

/// A Windows program's window, as the Windows host lists it: opened hidden, since showing one
/// captures it, and launched outside the desktop, so its program id is all that names its program.
fn windows_window(id: u64, program_id: &str, title: &str) -> TestWindow {
    TestWindow {
        opens_hidden: true,
        ..program_window(id, program_id, title, WindowOrigin::default())
    }
}

/// The Windows host's Start menu: each shortcut's windows carry its AppUserModelID, else its
/// target (`win32_map::host_program`).
fn start_menu() -> ProgramCatalog {
    let program = |id: &str, name: &str, wm_class: &str| HostProgram {
        id: id.to_owned(),
        name: name.to_owned(),
        generic_name: String::new(),
        comment: String::new(),
        keywords: Vec::new(),
        wm_class: Some(wm_class.to_owned()),
        icon: None,
    };
    ProgramCatalog {
        listed: true,
        programs: vec![
            program(
                "Accessories/Paint",
                "Paint",
                "C:\\Windows\\System32\\mspaint.exe",
            ),
            program(
                "Calculator",
                "Calculator",
                "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            ),
        ],
        shell: vec!["cmd.exe".to_owned(), "/D".to_owned(), "/C".to_owned()],
    }
}

/// On Windows, as on Linux, every window running when the desktop starts has a chip, and the
/// windows from last time wait in chips behind a Restore chip, which starts each from its Start
/// menu shortcut, matched by its AppUserModelID or its executable.
#[test]
fn windows_from_last_time_on_windows_wait_in_chips_and_restore_from_the_start_menu() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[
        windows_window(3, "C:\\Windows\\System32\\mspaint.exe", "Untitled - Paint"),
        windows_window(
            4,
            "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App",
            "Calculator",
        ),
    ]);
    for id in [3, 4] {
        assert!(
            before.test.shown(&format!("desktop.windows.{id}")),
            "window {id}, running when the desktop started, has no chip"
        );
    }
    let [paint, calculator] = [3, 4].map(|id| before.tab_of(id));

    let mut resumed = Fixture::resumed(before.saved());
    resumed
        .test
        .set_watched::<ProgramsWatch>(&start_menu());
    resumed.settle();
    assert!(resumed.test.shown("desktop.windows.restore"));
    for tab in [paint, calculator] {
        assert!(
            resumed
                .test
                .shown(&format!("desktop.windows.placeholder.{tab}")),
            "the window in tab {tab} from last time has no chip"
        );
    }
    resumed.test.settle();
    resumed
        .test
        .snapshot("windows_from_last_time_on_windows_wait_in_chips");

    resumed.test.clear_calls();
    resumed.test.click("desktop.windows.restore");
    resumed.settle();
    assert_eq!(
        resumed
            .test
            .take_host_calls(be_sdk_test::requested::program),
        vec![
            ProgramAction::Launch {
                program: "Accessories/Paint".to_owned(),
                launch: Some(paint),
            },
            ProgramAction::Launch {
                program: "Calculator".to_owned(),
                launch: Some(calculator),
            },
        ],
        "Restore starts each from its shortcut, by its executable or its AppUserModelID"
    );
}
