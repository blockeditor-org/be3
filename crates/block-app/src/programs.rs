use block_plugin_api::{ProgramAction, ProgramsWatch};

use crate::{plugin_host, wayland};

#[derive(Default)]
pub(crate) struct Programs {
    programs: wayland::Programs,
    offered: bool,
}

impl Programs {
    pub(crate) fn frame(&mut self) {
        for action in plugin_host::take_calls(block_plugin_api::requested::program) {
            match action {
                ProgramAction::List { icon_size } if wayland::running() => {
                    self.programs
                        .scan(icon_size.clamp(1, ProgramAction::MAX_ICON_SIZE));
                }
                ProgramAction::List { .. } => {}
                ProgramAction::Launch { program, launch } => {
                    self.programs.launch(&program, launch);
                }
                ProgramAction::Run(line) => {
                    wayland::launch(line);
                }
                ProgramAction::Exec { command, launch } => {
                    wayland::exec(command, launch);
                }
            }
        }
        let offering = !self.offered && wayland::running();
        self.offered |= offering;
        if self.programs.receive() || offering {
            plugin_host::publish::<ProgramsWatch>(&self.programs.listed());
        }
    }
}
