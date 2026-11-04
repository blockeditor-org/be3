use super::*;
use be_sdk_beui::{
    HostProgram, OpenProgram, OpenProgramsWatch, ProgramAction, ProgramCatalog, ProgramsWatch,
};

#[test]
fn a_program_group_comes_from_a_window_open_now() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));
    editor.set_watched::<OpenProgramsWatch>(&vec![
        OpenProgram {
            program_id: "steam_app_367520".to_owned(),
            command: vec!["Z:\\games\\Hollow Knight\\hollow_knight.exe".to_owned()],
        },
        OpenProgram {
            program_id: "org.gnome.Console".to_owned(),
            command: vec!["/usr/bin/kgx".to_owned()],
        },
    ]);
    editor.set_watched::<ProgramsWatch>(&ProgramCatalog {
        listed: true,
        programs: vec![
            HostProgram {
                id: "org.gnome.Console.desktop".to_owned(),
                name: "Console".to_owned(),
                ..HostProgram::default()
            },
            HostProgram {
                id: "org.gimp.GIMP.desktop".to_owned(),
                name: "GIMP".to_owned(),
                ..HostProgram::default()
            },
        ],
    });
    editor.run();

    editor.click("input-remaps.add-program");
    editor.run();
    assert!(
        matches!(
            editor.take_host_calls(be_sdk_test::requested::program)[..],
            [ProgramAction::List { .. }]
        ),
        "the dialog asks the host for the installed programs"
    );
    editor.click("input-remaps.programs.open.0");
    editor.run();
    editor.snapshot("the_program_dialog_lists_open_and_installed_programs");
    editor.click("input-remaps.programs.confirm");
    editor.run();

    assert!(!editor.shown("input-remaps.programs"));
    let text = stored(&editor);
    assert!(
        text.contains(
            "name = \"hollow_knight\"\nrules = []\nwindow = { exe = \"hollow_knight.exe\" }"
        ),
        "a game run through Proton is matched by its executable: {text}"
    );

    editor.click("input-remaps.group.1.scope");
    editor.run();
    editor.click("input-remaps.programs.installed.0");
    editor.run();
    editor.click("input-remaps.programs.confirm");
    editor.run();
    let text = stored(&editor);
    assert!(
        text.contains("name = \"GIMP\"\nrules = []\nwindow = { program_id = \"org.gimp.GIMP\" }"),
        "changing where a group applies reuses the dialog: {text}"
    );
}
