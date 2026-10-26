use beui::KeyAt;
use block_editor_beui::linux_desktop;
use block_editor_beui::beui::icons::{ICON_LAYOUT_GRID, ICON_TERMINAL};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Image, Key, NodeId};
use block_editor_beui::{Editor, HostProgram, ProgramAction, ProgramsWatch};

const ICON_POINTS: f32 = 32.0;
const RUN: &str = "run";

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

    fn choose(&self, key: String, query: &str) {
        match key == RUN {
            true => self.run(query.trim().to_owned()),
            false => self.launch(key),
        }
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

fn item(program: &HostProgram) -> PaletteItem {
    let detail = match program.comment.is_empty() {
        true => program.generic_name.clone(),
        false => program.comment.clone(),
    };
    let id = program.id.trim_end_matches(".desktop").to_owned();
    PaletteItem {
        key: program.id.clone(),
        title: program.name.clone(),
        detail,
        terms: std::iter::once(program.generic_name.clone())
            .chain(program.keywords.iter().cloned())
            .chain(std::iter::once(id))
            .collect(),
        glyph: Some(ICON_LAYOUT_GRID),
        image: program
            .icon
            .as_ref()
            .filter(|icon| icon.is_whole())
            .map(|icon| Image::from_rgba(icon.width, icon.height, icon.rgba.clone())),
        ..PaletteItem::default()
    }
}

fn run_item(query: &str) -> PaletteItem {
    PaletteItem {
        key: RUN.to_owned(),
        title: format!("Run \u{201c}{}\u{201d}", query.trim()),
        detail: "Run it as a command".to_owned(),
        glyph: Some(ICON_TERMINAL),
        ..PaletteItem::default()
    }
}

fn score(item: &PaletteItem, query: &str) -> Option<u32> {
    match item.key == RUN {
        true => Some(0),
        false => item.fuzzy_score(query),
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
    let query = create_signal(String::new());
    let items = create_memo(move || {
        let query = query.get();
        programs.with(|programs| {
            programs
                .iter()
                .map(item)
                .chain((!query.trim().is_empty()).then(|| run_item(&query)))
                .collect::<Vec<_>>()
        })
    });
    let open = Binding::new(launcher.open, move |open: bool| launcher.show(open));
    view! {
        <CommandPalette
            open
            items={PaletteItems::list(items)}
            score
            query
            placeholder="Search programs, or type a command"
            label="Search programs"
            id="launcher"
            on_choose={move |key: String| launcher.choose(key, &query.get_untracked())}
        />
    }
}
