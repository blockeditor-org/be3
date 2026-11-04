//! The dialog that says where a group's remaps apply: a program picked from
//! the ones open now or installed, or a pattern for the rare cases a list
//! can't name.

use be_host_remap::rules::exe_name;
use be_sdk_beui::beui::icons::{ICON_APP_WINDOW, ICON_LAYOUT_GRID, ICON_SEARCH};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{ItemSize, NodeId};
use be_sdk_beui::desktop;
use be_sdk_beui::{
    Editor, HostProgram, OpenProgram, OpenProgramsWatch, ProgramAction, ProgramsWatch,
};

const SPACING: Space = Space::Lg;
const FIELD_SPACING: Space = Space::Xs2;
const ICON_SIZE: u32 = 16;
/// How many installed programs show before a search narrows them.
const INSTALLED_SHOWN: usize = 30;

/// Where a group applies: its name and the patterns its window must match,
/// each empty for any.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(super) struct Scope {
    pub(super) name: String,
    pub(super) exe: String,
    pub(super) program_id: String,
    pub(super) title: String,
}

/// What the dialog is open for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scoping {
    Add,
    Group(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Listed {
    key: String,
    /// The id the program's windows carry, which may be matched by something else.
    id: String,
    name: String,
    detail: String,
    scope: Scope,
}

fn open_program(program: &OpenProgram, installed: &[HostProgram]) -> Listed {
    let exe = exe_name(&program.command);
    let known = installed
        .iter()
        .find(|known| !program.program_id.is_empty() && program_id(known) == program.program_id);
    let name = match (known, exe.is_empty()) {
        (Some(known), _) => known.name.clone(),
        (None, false) => exe
            .rsplit_once('.')
            .filter(|(_, extension)| extension.eq_ignore_ascii_case("exe"))
            .map_or(exe.clone(), |(stem, _)| stem.to_owned()),
        (None, true) => program.program_id.clone(),
    };
    let detail = [exe.as_str(), program.program_id.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    let scope = match exe.is_empty() {
        true => Scope {
            name: name.clone(),
            program_id: program.program_id.clone(),
            ..Scope::default()
        },
        false => Scope {
            name: name.clone(),
            exe: exe.clone(),
            ..Scope::default()
        },
    };
    Listed {
        key: format!("open:{}:{exe}", program.program_id),
        id: program.program_id.clone(),
        name,
        detail,
        scope,
    }
}

/// The id a program's windows carry: its window class, or its desktop file's name.
fn program_id(program: &HostProgram) -> String {
    program
        .wm_class
        .clone()
        .unwrap_or_else(|| program.id.trim_end_matches(".desktop").to_owned())
}

fn installed_program(program: &HostProgram) -> Listed {
    let id = program_id(program);
    Listed {
        key: format!("installed:{}", program.id),
        id: id.clone(),
        name: program.name.clone(),
        detail: id.clone(),
        scope: Scope {
            name: program.name.clone(),
            program_id: id,
            ..Scope::default()
        },
    }
}

fn matches(listed: &Listed, query: &str) -> bool {
    query.is_empty()
        || listed.name.to_lowercase().contains(query)
        || listed.detail.to_lowercase().contains(query)
}

#[component]
pub(super) fn ProgramDialog(
    editor: Editor,
    scoping: Signal<Option<Scoping>>,
    current: Callback<usize, Scope>,
    apply: Callback<(Scoping, Scope)>,
) -> NodeId {
    let open_offered = editor.offered::<OpenProgramsWatch>();
    let installed_offered = editor.offered::<ProgramsWatch>();
    let windows = editor.watch::<OpenProgramsWatch>();
    let catalog = editor.watch::<ProgramsWatch>();
    let listing = create_memo(move || open_offered.get() || installed_offered.get());
    let open = create_memo(move || scoping.get().is_some());
    let query = create_signal(String::new());
    let chosen = create_signal(None::<String>);
    let name = create_signal(String::new());
    let exe = create_signal(String::new());
    let program = create_signal(String::new());
    let title = create_signal(String::new());
    let patterns = create_signal(false);
    create_effect(move || {
        let Some(now) = scoping.get() else {
            return;
        };
        untrack(|| {
            let scope = match now {
                Scoping::Add => Scope::default(),
                Scoping::Group(group) => current.call(group),
            };
            query.set(String::new());
            chosen.set(None);
            patterns.set(!listing.get() || !scope.title.is_empty());
            name.set(scope.name);
            exe.set(scope.exe);
            program.set(scope.program_id);
            title.set(scope.title);
            if installed_offered.get() {
                editor.act(move || {
                    desktop::programs::request(ProgramAction::List {
                        icon_size: ICON_SIZE,
                    })
                    .detach()
                });
            }
        });
    });
    let running = create_memo(move || {
        let query = query.get().trim().to_lowercase();
        catalog.with(|catalog| {
            windows.with(|windows| {
                windows
                    .iter()
                    .map(|window| open_program(window, &catalog.programs))
                    .filter(|listed| matches(listed, &query))
                    .collect::<Vec<_>>()
            })
        })
    });
    let installed = create_memo(move || {
        let query = query.get().trim().to_lowercase();
        let running = running.get();
        catalog.with(|catalog| {
            let mut listed: Vec<Listed> = catalog
                .programs
                .iter()
                .map(installed_program)
                .filter(|listed| !running.iter().any(|open| !open.id.is_empty() && open.id == listed.id))
                .filter(|listed| matches(listed, &query))
                .collect();
            listed.sort_by_key(|listed| listed.name.to_lowercase());
            listed
        })
    });
    let shown = create_memo(move || {
        installed.with(|installed| installed.iter().take(INSTALLED_SHOWN).cloned().collect::<Vec<_>>())
    });
    let more = create_memo(move || installed.with(Vec::len) > INSTALLED_SHOWN);
    let any_running = create_memo(move || running.with(|running| !running.is_empty()));
    let any_installed = create_memo(move || shown.with(|shown| !shown.is_empty()));
    let none = create_memo(move || {
        listing.get() && !any_running.get() && !any_installed.get() && catalog.with(|catalog| catalog.listed || !installed_offered.get())
    });
    let choose = Callback::new(move |listed: Listed| {
        chosen.set(Some(listed.key));
        name.set(listed.scope.name);
        exe.set(listed.scope.exe);
        program.set(listed.scope.program_id);
        title.set(listed.scope.title);
    });
    let heading = create_memo(move || {
        match scoping.get() {
            Some(Scoping::Group(_)) => "Where these remaps apply",
            _ => "Add remaps for a program",
        }
        .to_owned()
    });
    let confirm_label = create_memo(move || {
        match scoping.get() {
            Some(Scoping::Group(_)) => "Save",
            _ => "Add",
        }
        .to_owned()
    });
    let unnamed = create_memo(move || name.with(|name| name.trim().is_empty()));
    let confirm = move || {
        if let Some(now) = scoping.get_untracked() {
            scoping.set(None);
            apply.call((
                now,
                Scope {
                    name: name.get_untracked().trim().to_owned(),
                    exe: exe.get_untracked().trim().to_owned(),
                    program_id: program.get_untracked().trim().to_owned(),
                    title: title.get_untracked().trim().to_owned(),
                },
            ));
        }
    };
    let summary = create_memo(move || {
        let named: Vec<String> = [
            ("program", exe.get()),
            ("program id", program.get()),
            ("title", title.get()),
        ]
        .into_iter()
        .filter(|(_, value)| !value.trim().is_empty())
        .map(|(what, value)| format!("{what} {value}"))
        .collect();
        match named.is_empty() {
            true => "Every program".to_owned(),
            false => named.join(", "),
        }
    });
    view! {
        <Dialog
            open={open}
            title={heading}
            id="input-remaps.programs"
            actions={vec![
                DialogAction::cancel("Cancel"),
                DialogAction::primary(confirm_label, confirm).disabled(unnamed),
            ]}
            on_dismiss={move || scoping.set(None)}
        >
            <List spacing=SPACING>
                <Text style=TextStyle::PARAGRAPH>
                    "They apply only while that program's window has the keyboard."
                </Text>
                <Show condition={listing}>
                    <List spacing=SPACING>
                        <TextInput
                            value={query}
                            placeholder="Search programs"
                            label="Search programs"
                            glyph={Some(ICON_SEARCH)}
                            clearable=true
                            @test_id={"input-remaps.programs.search"}
                        />
                        <Show condition={any_running}>
                            <ProgramList
                                title="Open now"
                                kind="open"
                                listed={running}
                                chosen={chosen}
                                choose={choose}
                            />
                        </Show>
                        <Show condition={any_installed}>
                            <ProgramList
                                title="Installed"
                                kind="installed"
                                listed={shown}
                                chosen={chosen}
                                choose={choose}
                            />
                        </Show>
                        <Show condition={more}>
                            <Text style=TextStyle::CAPTION>"Search to find more programs."</Text>
                        </Show>
                        <Show condition={none}>
                            <Text style=TextStyle::CAPTION>"No program matches."</Text>
                        </Show>
                    </List>
                </Show>
                <Field label="Name">
                    <TextInput
                        value={name}
                        label="Name"
                        @test_id={"input-remaps.programs.name"}
                    />
                </Field>
                <Accordion
                    title="Match a pattern instead"
                    detail={summary}
                    size=TextSize::Base
                    open={patterns}
                    @test_id={"input-remaps.programs.patterns"}
                >
                    <List spacing=SPACING>
                        <Field label="Program">
                            <TextInput
                                value={exe}
                                label="Program"
                                placeholder="Any, or like Celeste.exe"
                                @test_id={"input-remaps.programs.exe"}
                            />
                        </Field>
                        <Field label="Program id">
                            <TextInput
                                value={program}
                                label="Program id"
                                placeholder="Any"
                                @test_id={"input-remaps.programs.program-id"}
                            />
                        </Field>
                        <Field label="Window title">
                            <TextInput
                                value={title}
                                label="Window title"
                                placeholder="Any"
                                @test_id={"input-remaps.programs.title"}
                            />
                        </Field>
                        <Text style=TextStyle::CAPTION>
                            "* matches anything. Every field filled in must match."
                        </Text>
                    </List>
                </Accordion>
            </List>
        </Dialog>
    }
}

#[component]
fn Field(label: &'static str, children: Child) -> NodeId {
    view! {
        <List spacing=FIELD_SPACING>
            <Text style=TextStyle::CAPTION>{label}</Text>
            {children}
        </List>
    }
}

#[component]
fn ProgramList(
    title: &'static str,
    kind: &'static str,
    listed: Memo<Vec<Listed>>,
    chosen: Signal<Option<String>>,
    choose: Callback<Listed>,
) -> NodeId {
    let indices = create_memo(move || (0..listed.with(Vec::len)).collect::<Vec<usize>>());
    let glyph = match kind {
        "open" => ICON_APP_WINDOW,
        _ => ICON_LAYOUT_GRID,
    };
    view! {
        <List spacing=FIELD_SPACING>
            <Text style=TextStyle::CAPTION>{title}</Text>
            <List spacing=Space::None>
                <ForEach keys={indices}>|index: usize| {
                    let item = create_memo(move || listed.with(|listed| listed.get(index).cloned()));
                    let name = create_memo(move || item.get().map(|item| item.name).unwrap_or_default());
                    let detail = create_memo(move || item.get().map(|item| item.detail).unwrap_or_default());
                    let selected = create_memo(move || {
                        item.get().is_some_and(|item| chosen.get().as_deref() == Some(item.key.as_str()))
                    });
                    view! {
                        <ListRow
                            @sizing=ItemSize::Percent(100.0)
                            label={name}
                            detail={detail}
                            glyph={Some(glyph)}
                            selected={selected}
                            @test_id={format!("input-remaps.programs.{kind}.{index}")}
                            on_click={move || {
                                if let Some(item) = item.get_untracked() {
                                    choose.call(item);
                                }
                            }}
                        />
                    }
                }</ForEach>
            </List>
        </List>
    }
}
