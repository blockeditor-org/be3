use beui::NodeId;
use beui::icons::{ICON_KEY_ROUND, ICON_PLAY, ICON_TERMINAL};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::MenuItem;

use crate::builds::Slot;
use crate::github::{Entry, PullRequest, branch_deleted};
use crate::helper::Product;
use crate::model::{Loaded, Model};
#[cfg(target_os = "android")]
use crate::pane::TerminalPane;
use crate::runs::{Runs, Stage, Work};

const MEGABYTE: f64 = 1_000_000.0;
#[cfg(target_os = "android")]
const LOG_HEIGHT: f32 = 360.0;

#[cfg(target_os = "android")]
const MORE: [(&str, Product, bool); 2] = [
    ("Run with fresh data", Product::App, true),
    ("Install its launcher", Product::Launcher, false),
];
#[cfg(not(target_os = "android"))]
const MORE: [(&str, Product, bool); 1] = [("Run beui's demo", Product::Demo, false)];

#[component]
fn RunButton(
    model: Model,
    slot: Slot,
    commit: Memo<Option<String>>,
    label: &'static str,
    variant: ButtonVariant,
    refused: Memo<bool>,
) -> NodeId {
    let runs = model.runs;
    let disabled = create_memo(move || runs.work.with(Option::is_some) || refused.get());
    let run = move || runs.run(slot, commit.get_untracked(), Product::App, false);
    let pick = move |path: Vec<usize>| {
        if let [index] = path.as_slice()
            && let Some((_, product, fresh)) = MORE.get(*index)
        {
            runs.run(slot, commit.get_untracked(), *product, *fresh);
        }
    };
    view! {
        <SplitButton
            label
            glyph=ICON_PLAY
            variant
            menu_label="More ways to run it"
            disabled
            items={view! {
                <ForEach keys={(0..MORE.len()).collect::<Vec<_>>()}>|index: usize|
                    <MenuItem label={MORE[index].0} />
                </ForEach>
            }}
            on_click={run}
            on_select={pick}
        />
    }
}

fn refused(
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> Memo<bool> {
    create_memo(move || {
        !pull_request.get().same_repository
            || match timeline.get() {
                Loaded::Ready(entries) => branch_deleted(&entries),
                _ => false,
            }
    })
}

#[component]
pub(crate) fn Actions(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> NodeId {
    let slot = Slot::PullRequest(pull_request.get_untracked().number);
    let commit = create_memo(move || Some(pull_request.get().head_sha));
    let refused = refused(pull_request, timeline);
    view! {
        <RunButton model slot commit label="Run" variant=ButtonVariant::Primary refused />
    }
}

#[component]
pub(crate) fn Notes(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> NodeId {
    let slot = Slot::PullRequest(pull_request.get_untracked().number);
    let fork = create_memo(move || !pull_request.get().same_repository);
    let deleted = create_memo(move || match timeline.get() {
        Loaded::Ready(entries) => branch_deleted(&entries),
        _ => false,
    });
    view! {
        <List spacing=8.0>
            <Show condition={fork}>
                <Text style=TextStyle::CAPTION>
                    "It comes from a fork, so the build helper does not build it."
                </Text>
            </Show>
            <Show condition={deleted}>
                <Text style=TextStyle::CAPTION>
                    "Its branch was deleted, so there is nothing to build."
                </Text>
            </Show>
            <SlotNotes model slot />
        </List>
    }
}

#[component]
pub(crate) fn MainActions(model: Model) -> NodeId {
    let commit = create_memo(|| None);
    let refused = create_memo(|| false);
    let runs = model.runs;
    let current = create_memo(move || runs.holds(Slot::Main));
    let summary = create_memo(move || {
        if current.get() {
            "main is the one installed.".to_owned()
        } else {
            "Builds run on the build helper and download only what changed.".to_owned()
        }
    });
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>{summary}</Text>
                <RunButton
                    model
                    slot={Slot::Main}
                    commit
                    label="Run main"
                    variant=ButtonVariant::Secondary
                    refused
                />
            </List>
            <SlotNotes model slot={Slot::Main} />
            <KeyEntry runs />
        </List>
    }
}

#[component]
fn KeyEntry(runs: Runs) -> DynamicSegment<ListChild> {
    let shown = runs.needs_key;
    let key = create_signal(String::new());
    let save = move || {
        let entered = key.get_untracked();
        if !entered.trim().is_empty() {
            runs.save_key(&entered);
            key.set(String::new());
        }
    };
    view! {
        <Show condition={shown}>
            <List spacing=8.0>
                <Text style=TextStyle::CAPTION>
                    "The build helper is behind the API gateway: enter its key to build."
                </Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={key}
                        placeholder="The API gateway's key"
                        label="The API gateway's key"
                    />
                    <Button
                        label="Save"
                        glyph=ICON_KEY_ROUND
                        variant=ButtonVariant::Secondary
                        on_click={save}
                    />
                </List>
            </List>
        </Show>
    }
}

pub(crate) fn describe(work: &Work) -> String {
    let what = work.product.title();
    match work.stage {
        Stage::Asking => format!("Asking the build helper for {what}"),
        Stage::Queued(0) => format!("{what} is next on the build helper"),
        Stage::Queued(ahead) => format!("{ahead} builds are ahead of {what}"),
        Stage::Building => format!("Building {what}"),
        Stage::Downloading { total: 0, .. } => format!("{what} is already here"),
        Stage::Downloading { done, total } => {
            format!("Downloading {} of {}", megabytes(done), megabytes(total))
        }
        #[cfg(target_os = "android")]
        Stage::Removing => "Removing the app and its data".to_owned(),
        #[cfg(target_os = "android")]
        Stage::Installing => format!("Installing {what}"),
        #[cfg(not(target_os = "android"))]
        Stage::Starting => format!("Starting {what}"),
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / MEGABYTE)
}

#[component]
fn SlotNotes(model: Model, slot: Slot) -> NodeId {
    let runs = model.runs;
    let working = create_memo(move || {
        runs.work
            .with(|work| work.as_ref().is_some_and(|work| work.slot == slot))
    });
    let progress = create_memo(move || {
        runs.work
            .get()
            .map(|work| describe(&work))
            .unwrap_or_default()
    });
    let problem = create_memo(move || match runs.problem.get() {
        Some((problem_slot, problem)) if problem_slot == slot => problem,
        _ => String::new(),
    });
    let failed = create_memo(move || !problem.get().is_empty());
    view! {
        <List spacing=8.0>
            <Show condition={working}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Progress width=18.0 label="Working on the build" />
                    <Text style=TextStyle::CAPTION>{progress}</Text>
                </List>
            </Show>
            <Show condition={failed}>
                <Text color=TextColor::Danger>{problem}</Text>
            </Show>
            <SlotLog model slot />
        </List>
    }
}

#[cfg(not(target_os = "android"))]
#[component]
fn SlotLog(model: Model, slot: Slot) -> DynamicSegment<ListChild> {
    let runs = model.runs;
    let logged = create_memo(move || runs.logged.get() == Some(slot));
    view! {
        <Show condition={logged}>
            <Button
                label="Show the log"
                glyph=ICON_TERMINAL
                variant=ButtonVariant::Ghost
                on_click={move || model.show_tab(crate::model::Tab::Log)}
            />
        </Show>
    }
}

#[cfg(target_os = "android")]
#[component]
fn SlotLog(model: Model, slot: Slot) -> NodeId {
    let runs = model.runs;
    let logged = create_memo(move || runs.logged.get() == Some(slot));
    let open = create_signal(false);
    let shown = create_memo(move || logged.get() && open.get());
    let label = create_memo(move || {
        if open.get() {
            "Hide the log"
        } else {
            "Show the log"
        }
        .to_owned()
    });
    let toggle = move || open.update(|open| *open = !*open);
    let pane = runs.pane;
    view! {
        <List spacing=8.0>
            <Show condition={logged}>
                <Button label glyph=ICON_TERMINAL variant=ButtonVariant::Ghost on_click={toggle} />
            </Show>
            <Show condition={shown}>
                <Frame @sizing=ItemSize::Fixed(LOG_HEIGHT)>
                    <TerminalPane pane />
                </Frame>
            </Show>
        </List>
    }
}
