use beui::icons::{
    ICON_CALL_SPLIT, ICON_CANCEL, ICON_DRAFT, ICON_MERGE, ICON_REFRESH, ICON_SEARCH,
};
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Prop, Show, Spacer, Text, clone,
    component, create_memo, view,
};
use beui::styled::theme::{FONT_BODY, FONT_SMALL};
use beui::styled::{
    Caption, Heading, Icon, IconButton, ListRow, Scroll, Separator, Spinner, Tabs, TextInput,
    use_theme,
};
use beui::unstyled::ChoiceOption;
use beui::{Color32, NodeId};

use crate::detail::Detail;
use crate::github::{Filter, Label, PullRequest, State};
use crate::model::{Loaded, Model};
use crate::runner::RunPanel;
use crate::time::{now, relative};

const SIDEBAR_WIDTH: f32 = 380.0;
const PADDING: f32 = 16.0;
const SPACING: f32 = 10.0;
const ROW_SPACING: f32 = 4.0;
const CHIP_PADDING_HORIZONTAL: f32 = 7.0;
const CHIP_PADDING_VERTICAL: f32 = 1.0;
const CHIP_RADIUS: u8 = 9;
const DETAIL_SHARE: f32 = 64.0;
const RUN_SHARE: f32 = 36.0;
pub(crate) const MERGED: Color32 = Color32::from_rgb(163, 113, 247);

#[component]
pub(crate) fn Launcher(model: Model) -> NodeId {
    let theme = use_theme();
    let sidebar = model.clone();
    let detail = model.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame
                @sizing=ItemSize::Fixed(SIDEBAR_WIDTH)
                color={theme.surface.clone()}
                padding_horizontal=PADDING
                padding_vertical=PADDING
            >
                <Sidebar model={sidebar} />
            </Frame>
            <Separator direction=Direction::Vertical />
            <Frame
                @sizing=ItemSize::Percent(100.0)
                padding_horizontal=PADDING
                padding_vertical=PADDING
            >
                <List spacing=PADDING>
                    <Detail @sizing=ItemSize::Percent(DETAIL_SHARE) model={detail} />
                    <Separator />
                    <RunPanel @sizing=ItemSize::Percent(RUN_SHARE) model />
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn Sidebar(model: Model) -> NodeId {
    let theme = use_theme();
    let repository = create_memo(clone!(model -> move || match model.repository.get() {
        Loaded::Loading => "Connecting to GitHub".to_owned(),
        Loaded::Ready(name) => name,
        Loaded::Failed(error) => error,
    }));
    let tab = create_memo(clone!(model -> move || match model.filter.get() {
        Filter::Open => 0,
        Filter::Closed => 1,
    }));
    let loading = create_memo(clone!(model -> move || model.listing.get() == Loaded::Loading));
    let error = create_memo(clone!(model -> move || match model.listing.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    }));
    let failed = create_memo(clone!(error -> move || !error.get().is_empty()));
    let pull_requests = model.pull_requests.clone();
    let visible = create_memo(clone!(model pull_requests -> move || {
        let query = model.query.get().to_lowercase();
        pull_requests
            .keys()
            .get()
            .into_iter()
            .filter(|number| {
                query.is_empty()
                    || pull_requests
                        .try_get(number)
                        .is_some_and(|pull_request| matches(&pull_request.get(), &query))
            })
            .collect::<Vec<u64>>()
    }));
    let empty = create_memo(clone!(visible loading failed -> move || {
        visible.get().is_empty() && !loading.get() && !failed.get()
    }));
    let empty_text = create_memo(
        clone!(model -> move || match (model.filter.get(), model.query.get().is_empty()) {
            (_, false) => "Nothing matches the filter".to_owned(),
            (Filter::Open, true) => "No open pull requests".to_owned(),
            (Filter::Closed, true) => "No closed pull requests".to_owned(),
        }),
    );
    let refresh = clone!(model -> move || model.refresh_list());
    let show = clone!(model -> move |index: usize| {
        model.show(if index == 0 { Filter::Open } else { Filter::Closed });
    });
    let set_query = model.set_query.clone();
    let query = model.query.clone();
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                    <Heading content="Pull requests" />
                    <Caption content={repository} />
                </List>
                <IconButton glyph=ICON_REFRESH label="Refresh" on_click={refresh} />
            </List>
            <Tabs
                options={view! {
                    <ChoiceOption label="Open" />
                    <ChoiceOption label="Closed" />
                }}
                selected={tab}
                on_change={show}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Icon glyph=ICON_SEARCH color={theme.text_muted.clone()} />
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={query}
                    placeholder="Filter by title, author, branch or number"
                    label="Filter pull requests"
                    on_change={move |query| set_query.set(query)}
                />
            </List>
            <Show condition={loading}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner width=20.0 label="Loading pull requests" />
                    <Caption content="Loading pull requests" />
                </List>
            </Show>
            <Show condition={failed}>
                <Text string={error} font_size=FONT_BODY color={theme.danger.clone()} wrap=true />
            </Show>
            <Show condition={empty}>
                <Caption content={empty_text} />
            </Show>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <ForEach keys={visible}>
                    {move |number: u64| {
                        let pull_request = pull_requests.get(&number);
                        let pull_request = create_memo(move || pull_request.get());
                        view! {
                            <PullRequestRow model={model.clone()} pull_request />
                        }
                    }}
                </ForEach>
            </Scroll>
        </List>
    }
}

fn matches(pull_request: &PullRequest, query: &str) -> bool {
    let query = query.trim_start_matches('#');
    pull_request.number.to_string().starts_with(query)
        || pull_request.title.to_lowercase().contains(query)
        || pull_request.author.login.to_lowercase().contains(query)
        || pull_request.branch.to_lowercase().contains(query)
        || pull_request
            .labels
            .iter()
            .any(|label| label.name.to_lowercase().contains(query))
}

#[component]
fn PullRequestRow(model: Model, pull_request: Memo<PullRequest>) -> NodeId {
    let theme = use_theme();
    let number = pull_request.get_untracked().number;
    let selected = model.selection.memo(Some(number));
    let title = create_memo(clone!(pull_request -> move || pull_request.get().title));
    let details = create_memo(clone!(pull_request -> move || {
        let pull_request = pull_request.get();
        format!(
            "#{} by {} · updated {}",
            pull_request.number,
            pull_request.author.login,
            relative(now(), pull_request.updated)
        )
    }));
    let state = create_memo(clone!(pull_request -> move || pull_request.get().state));
    let labels = create_memo(clone!(pull_request -> move || pull_request.get().labels));
    let checked_out = create_memo(clone!(model pull_request -> move || {
        let head = model.head.get();
        !head.is_empty() && head == pull_request.get().head_sha
    }));
    let has_tags = create_memo(clone!(labels checked_out -> move || {
        !labels.get().is_empty() || checked_out.get()
    }));
    view! {
        <ListRow selected on_click={move || model.select(number)}>
            <List direction=Direction::Horizontal spacing=8.0>
                <StateIcon state />
                <List @sizing=ItemSize::Percent(100.0) spacing=ROW_SPACING>
                    <Text string={title} font_size=FONT_BODY color={theme.text.clone()} wrap=true />
                    <Caption content={details} />
                    <Show condition={has_tags}>
                        <Labels labels checked_out />
                    </Show>
                </List>
            </List>
        </ListRow>
    }
}

#[component]
pub(crate) fn StateIcon(state: Memo<State>) -> NodeId {
    let theme = use_theme();
    let glyph = create_memo(clone!(state -> move || state_glyph(state.get()).to_owned()));
    let color = create_memo(move || match state.get() {
        State::Open => theme.success.get(),
        State::Draft => theme.text_muted.get(),
        State::Merged => MERGED,
        State::Closed => theme.danger.get(),
    });
    view! {
        <Icon glyph color />
    }
}

pub(crate) fn state_glyph(state: State) -> &'static str {
    match state {
        State::Open => ICON_CALL_SPLIT,
        State::Draft => ICON_DRAFT,
        State::Merged => ICON_MERGE,
        State::Closed => ICON_CANCEL,
    }
}

#[component]
pub(crate) fn Labels(labels: Memo<Vec<Label>>, checked_out: Memo<bool>) -> NodeId {
    let theme = use_theme();
    let keys = create_memo(clone!(labels -> move || (0..labels.get().len()).collect::<Vec<_>>()));
    view! {
        <List direction=Direction::Horizontal spacing=6.0>
            <Show condition={checked_out}>
                <Chip text="checked out" color={theme.accent.clone()} />
            </Show>
            <ForEach keys>
                {move |index: usize| {
                    let label = create_memo(clone!(labels -> move || labels.get().get(index).cloned()));
                    let text = create_memo(clone!(label -> move || label.get().map(|label| label.name).unwrap_or_default()));
                    let color = create_memo(move || label.get().map_or(Color32::from_gray(128), |label| label.color));
                    view! {
                        <Chip text color />
                    }
                }}
            </ForEach>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn Chip(text: Prop<String>, color: Prop<Color32>) -> NodeId {
    let ink = create_memo(clone!(color -> move || readable_on(color.get())));
    view! {
        <Frame
            color
            radius=CHIP_RADIUS
            padding_horizontal=CHIP_PADDING_HORIZONTAL
            padding_vertical=CHIP_PADDING_VERTICAL
        >
            <Text string={text} font_size=FONT_SMALL color={ink} />
        </Frame>
    }
}

pub(crate) fn readable_on(background: Color32) -> Color32 {
    let [red, green, blue, _] = background.to_array();
    let luminance = 0.299 * f32::from(red) + 0.587 * f32::from(green) + 0.114 * f32::from(blue);
    if luminance > 150.0 {
        Color32::from_rgb(20, 22, 26)
    } else {
        Color32::WHITE
    }
}
