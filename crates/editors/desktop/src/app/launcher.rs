use block_editor_beui::beui::icons::{ICON_LAYOUT_GRID, ICON_TERMINAL};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Image, NodeId};
use block_editor_beui::desktop;
use block_editor_beui::{Editor, HostProgram, ProgramAction, ProgramsWatch};

const ICON_POINTS: f32 = 32.0;
const RUN: &str = "run";
const COLUMNS: usize = 4;
const RESULTS_HEIGHT: f32 = 320.0;

#[derive(Clone, Copy)]
pub(crate) struct Programs {
    editor: Editor,
    scale: ReadSignal<f32>,
    query: Signal<String>,
    pub(crate) offered: Memo<bool>,
}

impl Programs {
    pub(crate) fn new(editor: Editor) -> Self {
        Self {
            editor,
            scale: pixels_per_point(),
            query: create_signal(String::new()),
            offered: editor.offered::<ProgramsWatch>(),
        }
    }

    pub(crate) fn opened(&self) {
        self.query.set(String::new());
        if !self.offered.get_untracked() {
            return;
        }
        let icon_size = (ICON_POINTS * self.scale.get_untracked()).ceil() as u32;
        self.editor
            .act(move || desktop::programs::request(ProgramAction::List { icon_size }));
    }

    fn choose(&self, key: String) {
        let query = self.query.get_untracked();
        let action = match key == RUN {
            true => ProgramAction::Run(query.trim().to_owned()),
            false => ProgramAction::Launch {
                program: key,
                launch: None,
            },
        };
        self.editor.act(move || desktop::programs::request(action));
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
pub(super) fn ProgramSearch(
    programs: Programs,
    focused: Memo<bool>,
    close: Callback<()>,
) -> NodeId {
    let listed = programs.editor.watch::<ProgramsWatch>();
    let query = programs.query;
    let searching = create_memo(move || !query.with(|query| query.trim().is_empty()));
    let items = create_memo(move || {
        let query = query.get();
        listed.with(|listed| {
            listed
                .iter()
                .map(item)
                .chain((!query.trim().is_empty()).then(|| run_item(&query)))
                .collect::<Vec<_>>()
        })
    });
    let columns = create_memo(move || match searching.get() {
        true => 1,
        false => COLUMNS,
    });
    view! {
        <SearchList
            items
            score
            query
            columns
            focused
            list_height=RESULTS_HEIGHT
            placeholder="Search programs, or type a command"
            label="Search programs"
            id="desktop.apps"
            on_choose={move |key: String| {
                programs.choose(key);
                close.call(());
            }}
        />
    }
}
