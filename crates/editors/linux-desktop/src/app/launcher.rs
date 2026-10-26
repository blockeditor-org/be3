use beui::KeyAt;
use block_editor_beui::linux_desktop;
use std::rc::Rc;

use block_editor_beui::beui::icons::ICON_LAYOUT_GRID;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Image, Key, NodeId};
use block_editor_beui::{Editor, HostProgram, ProgramAction, ProgramsWatch};

const ICON_POINTS: f32 = 32.0;

#[derive(Clone, Copy)]
pub(crate) struct ProgramLauncher {
    editor: Editor,
    open: Signal<bool>,
    scale: ReadSignal<f32>,
    pub(crate) offered: Memo<bool>,
}

impl ProgramLauncher {
    pub(crate) fn new(editor: Editor) -> Self {
        let open = create_signal(false);
        Self {
            editor,
            open,
            scale: pixels_per_point(),
            offered: editor.offered::<ProgramsWatch>(),
        }
    }

    pub(crate) fn show(&self, open: bool) {
        if open && !self.open.get_untracked() {
            let icon_size = (ICON_POINTS * self.scale.get_untracked()).ceil() as u32;
            self.editor
                .act(move || linux_desktop::programs::request(ProgramAction::List { icon_size }));
        }
        self.open.set(open);
    }

    fn toggle(&self) {
        self.show(!self.open.get_untracked());
    }

    fn launch(&self, id: String) {
        self.show(false);
        self.editor.act(move || {
            linux_desktop::programs::request(ProgramAction::Launch {
                program: id,
                launch: None,
            })
        });
    }

    fn run(&self, line: String) {
        self.show(false);
        self.editor
            .act(move || linux_desktop::programs::request(ProgramAction::Run(line)));
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
    Action::new("desktop.launcher.toggle", "Programs", move || {
        launcher.toggle();
    })
    .glyph(ICON_LAYOUT_GRID)
    .shortcut(Chord::tap(Key::Logo))
    .at(KeyAt::Window)
    .capture()
    .register();
    let programs = launcher.editor.watch::<ProgramsWatch>();
    let items = create_memo(move || {
        programs.with(|programs| Rc::new(programs.iter().map(item).collect::<Vec<_>>()))
    });
    let open = launcher.open;
    view! {
        <Launcher
            open={open}
            items={items}
            placeholder="Search programs, or type a command"
            on_launch={move |id: String| launcher.launch(id)}
            on_run={move |line: String| launcher.run(line)}
            on_close={move || launcher.show(false)}
        />
    }
}
