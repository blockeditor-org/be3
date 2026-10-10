use block_plugin_api::{ProgramAction, Programs as Listed};

use crate::{plugin_host, wayland};

#[derive(Default)]
pub(crate) struct Programs {
    programs: wayland::Programs,
}

impl Programs {
    pub(crate) fn frame(&mut self) {
        for action in plugin_host::take_actions::<ProgramAction>() {
            match action {
                ProgramAction::List { icon_size } if wayland::running() => {
                    self.programs
                        .scan(icon_size.clamp(1, ProgramAction::MAX_ICON_SIZE));
                }
                ProgramAction::List { .. } => {}
                ProgramAction::Launch(id) => {
                    self.programs.launch(&id);
                }
                ProgramAction::Run(line) => {
                    wayland::launch(line);
                }
            }
        }
        if self.programs.receive() {
            plugin_host::publish::<Listed>(&self.programs.listed());
        }
    }
}
