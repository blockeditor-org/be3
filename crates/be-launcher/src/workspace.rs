use beui::NodeId;
use beui::icons::{
    ICON_DOWNLOAD, ICON_FLASK_CONICAL, ICON_PLAY, ICON_REFRESH_CW, ICON_SEARCH, ICON_SQUARE,
    ICON_TERMINAL, ICON_WRENCH,
};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::{
    DockDocument, DockDocumentBody, DockDocumentTabs, DockEntry, DockPane, DockTab, MenuItem,
};

use crate::detail::Detail;
use crate::github::{Entry, PullRequest, branch_deleted};
use crate::model::{COMMON, Loaded, Model, Tab};
use crate::pane::TerminalPane;
use crate::targets::{Action, Target};
use crate::view::{PADDING, state_glyph};
use beui::TextAlign;

const SPACING: f32 = 8.0;
const TARGET_ROW: f32 = 44.0;
const MAIN: &str = "main";

#[component]
pub(crate) fn Workspace(model: Model) -> NodeId {
    let head = create_memo(move || {
        let summary = model.head_summary.get();
        if summary.is_empty() {
            "Reading the checkout".to_owned()
        } else {
            format!("Checked out {summary}")
        }
    });
    let running = model.running;
    let idle = create_memo(move || !running.get());
    let stop = move || model.stop();
    let run = move || model.run_again();
    let pane = model.pane;
    view! {
        <List spacing=0.0>
            <Frame padding={Sides::symmetric(PADDING, 12.0)}>
                <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                    <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0) ellipsis=true>
                        {head}
                    </Text>
                    <Show condition={running}>
                        <Spinner width=18.0 label="A command is running" />
                    </Show>
                    <Show condition={running}>
                        <Button
                            label="Stop"
                            glyph=ICON_SQUARE
                            variant=ButtonVariant::Secondary
                            on_click={stop}
                        />
                    </Show>
                    <Show condition={idle}>
                        <Button
                            label="Run"
                            glyph=ICON_PLAY
                            variant=ButtonVariant::Primary
                            on_click={run}
                        />
                    </Show>
                </List>
            </Frame>
            <Docking @sizing=ItemSize::Percent(100.0) layout={model.layout}>
                <DockPane id="workspace" active={Some(Tab::Log)}>
                    <DockTab id=Tab::Targets title="Targets" icon=ICON_WRENCH>
                        <TargetList model />
                    </DockTab>
                    <DockTab id=Tab::Log title="Log" icon=ICON_TERMINAL>
                        <Frame padding=PADDING>
                            <TerminalPane pane />
                        </Frame>
                    </DockTab>
                    <DockDocumentTabs
                        documents={model.documents}
                    >|document: DockDocument<Tab, PullRequest, u64>|
                        <PullRequestTab model document />
                    </DockDocumentTabs>
                </DockPane>
            </Docking>
        </List>
    }
}

#[component]
fn PullRequestTab(model: Model, document: DockDocument<Tab, PullRequest, u64>) -> DockEntry<Tab> {
    let pull_request = document.content();
    let title = create_memo(move || {
        pull_request.with(|pull_request| format!("#{} {}", pull_request.number, pull_request.title))
    });
    let icon = create_memo(move || {
        state_glyph(pull_request.with(|pull_request| pull_request.state)).to_owned()
    });
    view! {
        <DockTab id={document.key()} title icon on_close={move || document.close()}>
            <DockDocumentBody document>|pull_request: ReadSignal<PullRequest>|
                <Detail model pull_request={create_memo(move || pull_request.get())} />
            </DockDocumentBody>
        </DockTab>
    }
}

#[component]
fn TargetList(model: Model) -> NodeId {
    let loading = create_memo(move || model.targets.get() == Some(Loaded::Loading));
    let error = create_memo(move || match model.targets.get() {
        Some(Loaded::Failed(error)) => error,
        _ => String::new(),
    });
    let failed = create_memo(move || !error.get().is_empty());
    let listed = create_memo(move || match model.targets.get() {
        Some(Loaded::Ready(targets)) => targets,
        _ => Vec::new(),
    });
    let shown = create_memo(move || {
        let query = model.target_query.get().to_lowercase();
        listed
            .get()
            .into_iter()
            .filter(|target| {
                query.is_empty()
                    || target.label.to_lowercase().contains(&query)
                    || target.kind.contains(&query)
            })
            .collect::<Vec<Target>>()
    });
    let keys = create_memo(move || {
        shown
            .get()
            .into_iter()
            .map(|target| target.label)
            .collect::<Vec<String>>()
    });
    let count = create_memo(move || {
        format!(
            "{} of {} targets, run on the current checkout",
            shown.get().len(),
            listed.get().len()
        )
    });
    let query = model.target_query;
    let refresh = move || model.refresh_targets();
    view! {
        <List spacing=0.0>
            <Frame padding={Sides::symmetric(PADDING, 4.0)}>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text color=TextColor::Muted align=TextAlign::Center>
                            <Icon glyph=ICON_SEARCH />
                        </Text>
                        <TextInput
                            @sizing=ItemSize::Percent(100.0)
                            value={query}
                            placeholder="Filter targets, like beui or rust_test"
                            label="Filter targets"
                        />
                        <IconButton
                            glyph=ICON_REFRESH_CW
                            label="List the targets again"
                            on_click={refresh}
                        />
                    </List>
                    <Text style=TextStyle::CAPTION>{count}</Text>
                    <Show condition={loading}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Spinner width=20.0 label="Listing targets" />
                            <Text style=TextStyle::CAPTION>"Asking buck2 for the targets"</Text>
                        </List>
                    </Show>
                    <Show condition={failed}>
                        <Text color=TextColor::Danger wrap=true>{error}</Text>
                    </Show>
                </List>
            </Frame>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <VirtualList keys item_size=TARGET_ROW>|label: ReadSignal<String>| {
                    let target = create_memo(move || {
                        label.with(|label| {
                            shown.with(|shown| {
                                shown.iter().find(|target| &target.label == label).cloned()
                            })
                        })
                    });
                    view! {
                        <TargetRow model={model} target />
                    }
                }</VirtualList>
            </Scroll>
        </List>
    }
}

#[component]
fn TargetRow(model: Model, target: Memo<Option<Target>>) -> NodeId {
    let label = create_memo(move || target.get().map(|target| target.label).unwrap_or_default());
    let kind = create_memo(move || target.get().map(|target| target.kind).unwrap_or_default());
    let action = create_memo(move || target.get().map_or(Action::Build, |target| target.action));
    let verb = create_memo(move || {
        match action.get() {
            Action::Run => "Run",
            Action::Test => "Test",
            Action::Build => "Build",
        }
        .to_owned()
    });
    let glyph = create_memo(move || {
        match action.get() {
            Action::Run => ICON_PLAY,
            Action::Test => ICON_FLASK_CONICAL,
            Action::Build => ICON_WRENCH,
        }
        .to_owned()
    });
    let buildable = create_memo(move || action.get() != Action::Build);
    let primary = move || {
        if let Some(target) = target.get_untracked() {
            model.run_target(&target, action.get_untracked());
        }
    };
    let build = move || {
        if let Some(target) = target.get_untracked() {
            model.run_target(&target, Action::Build);
        }
    };
    let busy = model.running;
    view! {
        <Frame padding={Sides::symmetric(PADDING, 4.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Text style=TextStyle::CODE @sizing=ItemSize::Percent(100.0)>{label}</Text>
                <Text style=TextStyle::CAPTION>{kind}</Text>
                <Show condition={buildable}>
                    <Button
                        label="Build"
                        glyph=ICON_WRENCH
                        variant=ButtonVariant::Ghost
                        disabled={busy}
                        on_click={build}
                    />
                </Show>
                <Button
                    label={verb}
                    glyph
                    variant=ButtonVariant::Secondary
                    disabled={busy}
                    on_click={primary}
                />
            </List>
        </Frame>
    }
}

#[component]
pub(crate) fn Actions(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> NodeId {
    let fork = create_memo(move || !pull_request.get().same_repository);
    let deleted = create_memo(move || match timeline.get() {
        Loaded::Ready(entries) => branch_deleted(&entries),
        _ => false,
    });
    let busy = create_memo(move || model.running.get() || fork.get() || deleted.get());
    let check_out = move || model.check_out(&pull_request.get_untracked(), None);
    let check_out_and_run = move || {
        model.check_out(
            &pull_request.get_untracked(),
            Some(model.common.get_untracked()),
        );
    };
    let pick = move |path: Vec<usize>| match path.as_slice() {
        [index] if *index < COMMON.len() => {
            model.check_out(&pull_request.get_untracked(), Some(*index));
        }
        _ => model.show_tab(Tab::Targets),
    };
    let run_label = create_memo(move || {
        let common = COMMON.get(model.common.get()).unwrap_or(&COMMON[0]);
        format!("Check out and run {}", common.title)
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
            <SplitButton
                label={run_label}
                glyph=ICON_PLAY
                variant=ButtonVariant::Primary
                menu_label="Pick what to run"
                disabled={busy}
                items={view! {
                    <ForEach keys={(0..COMMON.len()).collect::<Vec<_>>()}>|index: usize|
                        <MenuItem label={format!("Run {}", COMMON[index].title)} />
                    </ForEach>
                    <MenuItem label="All targets" />
                }}
                on_click={check_out_and_run}
                on_select={pick}
            />
            <Button
                label="Check out"
                glyph=ICON_DOWNLOAD
                variant=ButtonVariant::Secondary
                disabled={busy}
                on_click={check_out}
            />
        </List>
    }
}

#[component]
pub(crate) fn Notes(
    model: Model,
    pull_request: Memo<PullRequest>,
    timeline: ReadSignal<Loaded<Vec<Entry>>>,
) -> DynamicSegment<ListChild> {
    let _ = model;
    let fork = create_memo(move || !pull_request.get().same_repository);
    let deleted = create_memo(move || match timeline.get() {
        Loaded::Ready(entries) => branch_deleted(&entries),
        _ => false,
    });
    let shown = create_memo(move || fork.get() || deleted.get());
    view! {
        <Show condition={shown}>
            <List spacing=8.0>
                <Show condition={fork}>
                    <Text style=TextStyle::CAPTION wrap=true>
                        "This pull request comes from a fork, so scripts/switch cannot check it out."
                    </Text>
                </Show>
                <Show condition={deleted}>
                    <Text style=TextStyle::CAPTION wrap=true>
                        "Its branch was deleted, so there is nothing to check out."
                    </Text>
                </Show>
            </List>
        </Show>
    }
}

#[component]
pub(crate) fn MainActions(model: Model) -> NodeId {
    let busy = model.running;
    let run = move || model.switch(MAIN, Some(model.common.get_untracked()));
    let pick = move |path: Vec<usize>| match path.as_slice() {
        [index] if *index < COMMON.len() => model.switch(MAIN, Some(*index)),
        _ => model.switch(MAIN, None),
    };
    let run_label = create_memo(move || {
        let common = COMMON.get(model.common.get()).unwrap_or(&COMMON[0]);
        format!("Run {} on main", common.title)
    });
    view! {
        <SplitButton
            label={run_label}
            glyph=ICON_PLAY
            variant=ButtonVariant::Secondary
            menu_label="Pick what to run on main"
            disabled={busy}
            items={view! {
                <ForEach keys={(0..COMMON.len()).collect::<Vec<_>>()}>|index: usize|
                    <MenuItem label={format!("Run {}", COMMON[index].title)} />
                </ForEach>
                <MenuItem label="Check out main" />
            }}
            on_click={run}
            on_select={pick}
        />
    }
}
