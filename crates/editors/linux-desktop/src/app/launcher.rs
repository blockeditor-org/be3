use std::rc::Rc;

use block_editor_beui::beui::icons::ICON_APPS;
use block_editor_beui::beui::reactive::{
    Action, Chord, ReadSignal, WriteSignal, component, create_memo, create_signal,
    pixels_per_point, view,
};
use block_editor_beui::beui::styled::{Launcher, LauncherItem};
use block_editor_beui::beui::{Image, Key, NodeId};
use block_editor_beui::{Editor, HostProgram, ProgramAction, Programs};

const ICON_POINTS: f32 = 32.0;

#[derive(Clone)]
pub(crate) struct ProgramLauncher {
    editor: Editor,
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    scale: ReadSignal<f32>,
}

impl ProgramLauncher {
    pub(crate) fn new(editor: Editor) -> Self {
        let (open, set_open) = create_signal(false);
        Self {
            editor,
            open,
            set_open,
            scale: pixels_per_point(),
        }
    }

    pub(crate) fn show(&self, open: bool) {
        if open && !self.open.get_untracked() {
            let icon_size = (ICON_POINTS * self.scale.get_untracked()).ceil() as u32;
            self.editor.act(ProgramAction::List { icon_size });
        }
        self.set_open.set(open);
    }

    fn toggle(&self) {
        self.show(!self.open.get_untracked());
    }

    fn launch(&self, id: String) {
        self.show(false);
        self.editor.act(ProgramAction::Launch(id));
    }

    fn run(&self, line: String) {
        self.show(false);
        self.editor.act(ProgramAction::Run(line));
    }
}

fn item(program: &HostProgram) -> LauncherItem {
    let detail = match program.comment.is_empty() {
        true => program.generic_name.clone(),
        false => program.comment.clone(),
    };
    let id = program.id.trim_end_matches(".desktop").to_owned();
    LauncherItem {
        key: program.id.clone(),
        title: program.name.clone(),
        detail,
        terms: std::iter::once(program.generic_name.clone())
            .chain(program.keywords.iter().cloned())
            .chain(std::iter::once(id))
            .collect(),
        image: program
            .icon
            .as_ref()
            .filter(|icon| icon.is_whole())
            .map(|icon| Image::from_rgba(icon.width, icon.height, icon.rgba.clone())),
    }
}

#[component]
pub(super) fn ProgramLauncherOverlay(launcher: ProgramLauncher) -> NodeId {
    let toggling = launcher.clone();
    Action::new("desktop.launcher.toggle", "Programs", move || {
        toggling.toggle();
    })
    .glyph(ICON_APPS)
    .shortcut(Chord::tap(Key::Logo))
    .intercepts()
    .register();
    let programs = launcher.editor.host_value::<Programs>();
    let items = create_memo(move || {
        programs.with(|programs| Rc::new(programs.iter().map(item).collect::<Vec<_>>()))
    });
    let open = launcher.open.clone();
    let (launching, running, closing) = (launcher.clone(), launcher.clone(), launcher);
    view! {
        <Launcher
            open={open}
            items={items}
            placeholder="Search programs, or type a command"
            on_launch={move |id: String| launching.launch(id)}
            on_run={move |line: String| running.run(line)}
            on_close={move || closing.show(false)}
        />
    }
}
