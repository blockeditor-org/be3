use beui::icons::{
    ICON_BUILD, ICON_DOWNLOAD, ICON_PLAY_ARROW, ICON_REFRESH, ICON_SCIENCE, ICON_SEARCH, ICON_STOP,
};
use beui::reactive::{
    Align, Direction, DynamicSegment, ForEach, Frame, ItemSize, List, ListChild, Memo, ReadSignal,
    Show, Text, VirtualList, clone, component, create_memo, view,
};
use beui::styled::theme::FONT_BODY;
use beui::styled::{
    Button, ButtonVariant, Caption, Code, Icon, IconButton, MenuButton, Scroll, Spinner, Tabs,
    TextInput, use_theme,
};
use beui::unstyled::{ChoiceOption, MenuItem};
use beui::{NodeId, TextAlign};

use crate::detail::Detail;
use crate::github::{Entry, PullRequest, branch_deleted};
use crate::model::{COMMON, Loaded, Model, Tab};
use crate::pane::TerminalPane;
use crate::targets::{Action, Target};
use crate::view::PADDING;

const SPACING: f32 = 8.0;
const TARGET_ROW: f32 = 44.0;
const HEAD_CHARACTERS: usize = 48;
const MAIN: &str = "main";

#[component]
pub(crate) fn Workspace(model: Model) -> NodeId {
    let tab = create_memo(clone!(model -> move || {
        let tab = model.tab.get();
        Tab::ALL.iter().position(|candidate| *candidate == tab).unwrap_or(0)
    }));
    let head = create_memo(clone!(model -> move || {
        let summary = model.head_summary.get();
        if summary.is_empty() {
            "Reading the checkout".to_owned()
        } else {
            let mut shown: String = summary.chars().take(HEAD_CHARACTERS).collect();
            if shown.len() < summary.len() {
                shown.push_str("...");
            }
            format!("Checked out {shown}")
        }
    }));
    let running = model.running.clone();
    let idle = create_memo(clone!(running -> move || !running.get()));
    let show = clone!(model -> move |index: usize| {
        if let Some(tab) = Tab::ALL.get(index) {
            model.show_tab(*tab);
        }
    });
    let stop = clone!(model -> move || model.stop());
    let run = clone!(model -> move || model.run_again());
    let on_pull_request = create_memo(clone!(model -> move || model.tab.get() == Tab::PullRequest));
    let on_targets = create_memo(clone!(model -> move || model.tab.get() == Tab::Targets));
    let on_log = create_memo(clone!(model -> move || model.tab.get() == Tab::Log));
    let detail = model.clone();
    let targets = model.clone();
    let pane = model.pane.clone();
    view! {
        <List spacing=0.0>
            <Frame padding_horizontal=PADDING padding_vertical=12.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                    <Tabs
                        options={view! {
                            <ChoiceOption label="PR" />
                            <ChoiceOption label="Targets" />
                            <ChoiceOption label="Log" />
                        }}
                        selected={tab}
                        on_change={show}
                    />
                    <Caption @sizing=ItemSize::Percent(100.0) content={head} align=TextAlign::End />
                    <Show condition={running.clone()}>
                        <Spinner width=18.0 label="A command is running" />
                    </Show>
                    <Show condition={running.clone()}>
                        <Button
                            label="Stop"
                            glyph=ICON_STOP
                            variant=ButtonVariant::Secondary
                            on_click={stop}
                        />
                    </Show>
                    <Show condition={idle}>
                        <Button
                            label="Run"
                            glyph=ICON_PLAY_ARROW
                            variant=ButtonVariant::Primary
                            on_click={run}
                        />
                    </Show>
                </List>
            </Frame>
            <Show condition={on_pull_request}>
                <Detail @sizing=ItemSize::Percent(100.0) model={detail} />
            </Show>
            <Show condition={on_targets}>
                <TargetList @sizing=ItemSize::Percent(100.0) model={targets} />
            </Show>
            <Show condition={on_log}>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <TerminalPane pane />
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn TargetList(model: Model) -> NodeId {
    let theme = use_theme();
    let loading =
        create_memo(clone!(model -> move || model.targets.get() == Some(Loaded::Loading)));
    let error = create_memo(clone!(model -> move || match model.targets.get() {
        Some(Loaded::Failed(error)) => error,
        _ => String::new(),
    }));
    let failed = create_memo(clone!(error -> move || !error.get().is_empty()));
    let listed = create_memo(clone!(model -> move || match model.targets.get() {
        Some(Loaded::Ready(targets)) => targets,
        _ => Vec::new(),
    }));
    let shown = create_memo(clone!(model listed -> move || {
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
    }));
    let keys = create_memo(clone!(shown -> move || {
        shown.get().into_iter().map(|target| target.label).collect::<Vec<String>>()
    }));
    let count = create_memo(clone!(shown listed -> move || {
        format!(
            "{} of {} targets, run on the current checkout",
            shown.get().len(),
            listed.get().len()
        )
    }));
    let query = model.target_query.clone();
    let set_query = model.set_target_query.clone();
    let refresh = clone!(model -> move || model.refresh_targets());
    view! {
        <List spacing=0.0>
            <Frame padding_horizontal=PADDING padding_vertical=4.0>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Icon glyph=ICON_SEARCH color={theme.text_muted.clone()} />
                        <TextInput
                            @sizing=ItemSize::Percent(100.0)
                            value={query}
                            placeholder="Filter targets, like beui or rust_test"
                            label="Filter targets"
                            on_change={move |query| set_query.set(query)}
                        />
                        <IconButton
                            glyph=ICON_REFRESH
                            label="List the targets again"
                            on_click={refresh}
                        />
                    </List>
                    <Caption content={count} />
                    <Show condition={loading}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Spinner width=20.0 label="Listing targets" />
                            <Caption content="Asking buck2 for the targets" />
                        </List>
                    </Show>
                    <Show condition={failed}>
                        <Text
                            string={error}
                            font_size=FONT_BODY
                            color={theme.danger.clone()}
                            wrap=true
                        />
                    </Show>
                </List>
            </Frame>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <VirtualList keys item_size=TARGET_ROW>
                    {move |label: String| {
                        let shown = shown.clone();
                        let target = create_memo(move || {
                            shown.get().into_iter().find(|target| target.label == label)
                        });
                        view! {
                            <TargetRow model={model.clone()} target />
                        }
                    }}
                </VirtualList>
            </Scroll>
        </List>
    }
}

#[component]
fn TargetRow(model: Model, target: Memo<Option<Target>>) -> NodeId {
    let label = create_memo(clone!(target -> move || {
        target.get().map(|target| target.label).unwrap_or_default()
    }));
    let kind = create_memo(clone!(target -> move || {
        target.get().map(|target| target.kind).unwrap_or_default()
    }));
    let action = create_memo(clone!(target -> move || {
        target.get().map_or(Action::Build, |target| target.action)
    }));
    let verb = create_memo(clone!(action -> move || {
        match action.get() {
            Action::Run => "Run",
            Action::Test => "Test",
            Action::Build => "Build",
        }
        .to_owned()
    }));
    let glyph = create_memo(clone!(action -> move || {
        match action.get() {
            Action::Run => ICON_PLAY_ARROW,
            Action::Test => ICON_SCIENCE,
            Action::Build => ICON_BUILD,
        }
        .to_owned()
    }));
    let buildable = create_memo(clone!(action -> move || action.get() != Action::Build));
    let primary = clone!(model target action -> move || {
        if let Some(target) = target.get_untracked() {
            model.run_target(&target, action.get_untracked());
        }
    });
    let build = clone!(model target -> move || {
        if let Some(target) = target.get_untracked() {
            model.run_target(&target, Action::Build);
        }
    });
    let busy = model.running.clone();
    let busy_build = busy.clone();
    view! {
        <Frame padding_horizontal=PADDING padding_vertical=4.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Code @sizing=ItemSize::Percent(100.0) content={label} />
                <Caption content={kind} />
                <Show condition={buildable}>
                    <Button
                        label="Build"
                        glyph=ICON_BUILD
                        variant=ButtonVariant::Ghost
                        disabled={busy_build}
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
    let fork = create_memo(clone!(pull_request -> move || !pull_request.get().same_repository));
    let deleted = create_memo(move || match timeline.get() {
        Loaded::Ready(entries) => branch_deleted(&entries),
        _ => false,
    });
    let busy = create_memo(clone!(model -> move || {
        model.running.get() || fork.get() || deleted.get()
    }));
    let check_out =
        clone!(model pull_request -> move || model.check_out(&pull_request.get_untracked(), None));
    let check_out_and_run = clone!(model pull_request -> move || {
        model.check_out(&pull_request.get_untracked(), Some(model.common.get_untracked()));
    });
    let pick = clone!(model pull_request -> move |path: Vec<usize>| match path.as_slice() {
        [index] if *index < COMMON.len() => {
            model.check_out(&pull_request.get_untracked(), Some(*index));
        }
        _ => model.show_tab(Tab::Targets),
    });
    let busy_run = busy.clone();
    let busy_pick = busy.clone();
    let run_label = create_memo(clone!(model -> move || {
        let common = COMMON.get(model.common.get()).unwrap_or(&COMMON[0]);
        format!("Check out and run {}", common.title)
    }));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=1.0>
                <Button
                    label={run_label}
                    glyph=ICON_PLAY_ARROW
                    variant=ButtonVariant::Primary
                    disabled={busy_run}
                    on_click={check_out_and_run}
                />
                <MenuButton
                    label="Pick what to run"
                    variant=ButtonVariant::Primary
                    icon_only=true
                    disabled={busy_pick}
                    items={view! {
                        <ForEach keys={(0..COMMON.len()).collect::<Vec<_>>()}>
                            {|index: usize| view! {
                                <MenuItem label={format!("Run {}", COMMON[index].title)} />
                            }}
                        </ForEach>
                        <MenuItem label="All targets" />
                    }}
                    on_select={pick}
                />
            </List>
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
    let shown = create_memo(clone!(fork deleted -> move || fork.get() || deleted.get()));
    view! {
        <Show condition={shown}>
            <List spacing=8.0>
                <Show condition={fork.clone()}>
                    <Caption
                        content="This pull request comes from a fork, so scripts/switch cannot check it out."
                        wrap=true
                    />
                </Show>
                <Show condition={deleted.clone()}>
                    <Caption
                        content="Its branch was deleted, so there is nothing to check out."
                        wrap=true
                    />
                </Show>
            </List>
        </Show>
    }
}

#[component]
pub(crate) fn MainActions(model: Model) -> NodeId {
    let busy = model.running.clone();
    let busy_pick = busy.clone();
    let run = clone!(model -> move || model.switch(MAIN, Some(model.common.get_untracked())));
    let pick = clone!(model -> move |path: Vec<usize>| match path.as_slice() {
        [index] if *index < COMMON.len() => model.switch(MAIN, Some(*index)),
        _ => model.switch(MAIN, None),
    });
    let run_label = create_memo(clone!(model -> move || {
        let common = COMMON.get(model.common.get()).unwrap_or(&COMMON[0]);
        format!("Run {} on main", common.title)
    }));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=1.0>
            <Button
                label={run_label}
                glyph=ICON_PLAY_ARROW
                variant=ButtonVariant::Secondary
                disabled={busy}
                on_click={run}
            />
            <MenuButton
                label="Pick what to run on main"
                variant=ButtonVariant::Secondary
                icon_only=true
                disabled={busy_pick}
                items={view! {
                    <ForEach keys={(0..COMMON.len()).collect::<Vec<_>>()}>
                        {|index: usize| view! {
                            <MenuItem label={format!("Run {}", COMMON[index].title)} />
                        }}
                    </ForEach>
                    <MenuItem label="Check out main" />
                }}
                on_select={pick}
            />
        </List>
    }
}
