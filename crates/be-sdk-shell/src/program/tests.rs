use super::*;

mod a_launched_window_fills_the_tab_it_was_launched_for;
mod a_new_window_fills_the_oldest_placeholder_of_its_program;
mod a_window_that_leaves_closes_its_tab_unless_the_session_is_ending;

fn window(id: u64, program_id: &str, launch: Option<u64>) -> HostWindow {
    HostWindow {
        id: HostWindowId(id),
        kind: WindowKind::Window,
        parent: None,
        modal: false,
        program_id: program_id.to_owned(),
        origin: be_sdk_beui::WindowOrigin {
            program: None,
            command: vec![program_id.to_owned()],
            launch,
        },
        size: None,
        maximized: false,
    }
}

fn waiting(program_id: &str) -> Program {
    Program::restored(ProgramRecord {
        program_id: program_id.to_owned(),
        command: vec![program_id.to_owned()],
        ..ProgramRecord::default()
    })
}

fn bound(programs: &Programs) -> Vec<(u64, Option<u64>)> {
    programs
        .iter()
        .map(|(tab, program)| (tab.value(), program.window.map(|window| window.0)))
        .collect()
}
