use beui::icons::Glyph;
#[cfg(target_os = "android")]
use beui::icons::ICON_ARROW_LEFT;
use beui::icons::{
    ICON_CIRCLE_X, ICON_FILE, ICON_MERGE, ICON_REFRESH_CW, ICON_SCAN_SEARCH, ICON_SEARCH,
    ICON_SPLIT,
};
use beui::native::Document;
use beui::native::with_document;
use beui::reactive::*;
use beui::styled::*;
use beui::{Color32, NodeId};

use crate::actions::MainActions;
#[cfg(target_os = "android")]
use crate::detail::{Detail, placeholder};
use crate::github::{Filter, Label, PullRequest, State};
use crate::model::{Loaded, Model};
use crate::time::{now, relative};
use crate::viewer::ImageViewer;
#[cfg(not(target_os = "android"))]
use crate::workspace::Workspace;
use beui::TextAlign;
use beui::{Align, Direction, ItemSize};

#[cfg(not(target_os = "android"))]
const SIDEBAR_WIDTH: f32 = 380.0;
pub(crate) const PADDING: Space = Space::Xl;
const SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs;
pub(crate) const MERGED: Color32 = Color32::from_rgb(163, 113, 247);
#[cfg(not(target_os = "android"))]
const CURRENT: &str = "last run";
#[cfg(target_os = "android")]
const CURRENT: &str = "installed";

#[cfg(target_os = "android")]
#[component]
pub(crate) fn Launcher(model: Model) -> NodeId {
    let listing = create_memo(move || model.opened.with(Vec::is_empty));
    let opened = create_memo(move || {
        model.opened.with(|opened| {
            opened
                .iter()
                .map(|pull_request| pull_request.number)
                .collect::<Vec<u64>>()
        })
    });
    let reading = create_memo(move || !listing.get());
    let back = move || model.deselect();
    let sidebar = view! {
        <Sidebar model={model} />
    };
    let theme = use_theme();
    let page = theme.background;
    let shade = theme.scrim_soft;
    view! {
        <List spacing=Space::None>
            <Show condition={listing}>
                <Portal @sizing=ItemSize::Percent(100.0) node={Some(sidebar)} />
            </Show>
            <Show condition={reading}>
                <BackSlide
                    @sizing=ItemSize::Percent(100.0)
                    on_back={back}
                    shade={shade}
                    behind={move || view! {
                        <Portal node={Some(sidebar)} />
                    }}
                >
                    <Frame fill={ColorRole::custom(page)}>
                        <List spacing=Space::None>
                            <Frame padding={Padding::symmetric(Space::Md, Space::Xs)}>
                                <Button
                                    icon_only=true
                                    glyph=ICON_ARROW_LEFT
                                    label="Back to the pull requests"
                                    on_click={back}
                                />
                            </Frame>
                            <ForEach keys={opened}>|number: u64| {
                                let pull_request = create_memo(move || {
                                    model.opened.with(|opened| {
                                        opened
                                            .iter()
                                            .find(|pull_request| pull_request.number == number)
                                            .cloned()
                                            .unwrap_or_else(|| placeholder(number))
                                    })
                                });
                                view! {
                                    <Detail
                                        @sizing=ItemSize::Percent(100.0)
                                        model={model}
                                        pull_request
                                    />
                                }
                            }</ForEach>
                        </List>
                    </Frame>
                </BackSlide>
            </Show>
            <ImageViewer model={model} />
        </List>
    }
}

#[cfg(not(target_os = "android"))]
#[component]
pub(crate) fn Launcher(model: Model) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame @sizing=ItemSize::Fixed(SIDEBAR_WIDTH) fill=ColorRole::Surface>
                <Sidebar model={model} />
            </Frame>
            <Separator direction=Direction::Vertical />
            <Frame @sizing=ItemSize::Percent(100.0)>
                <Workspace model />
            </Frame>
            <ImageViewer model={model} />
        </List>
    }
}

#[component]
fn Sidebar(model: Model) -> NodeId {
    let repository = create_memo(move || match model.repository.get() {
        Loaded::Loading => "Connecting to GitHub".to_owned(),
        Loaded::Ready(name) => name,
        Loaded::Failed(error) => error,
    });
    let tab = create_memo(move || match model.filter.get() {
        Filter::Open => 0,
        Filter::Closed => 1,
    });
    let loading = create_memo(move || model.listing.get() == Loaded::Loading);
    let error = create_memo(move || match model.listing.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    });
    let failed = create_memo(move || !error.get().is_empty());
    let visible = create_memo(move || {
        let query = model.query.get().to_lowercase();
        let pull_requests = model.pull_requests;
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
    });
    let empty = create_memo(move || visible.get().is_empty() && !loading.get() && !failed.get());
    let empty_text =
        create_memo(
            move || match (model.filter.get(), model.query.get().is_empty()) {
                (_, false) => "Nothing matches the filter".to_owned(),
                (Filter::Open, true) => "No open pull requests".to_owned(),
                (Filter::Closed, true) => "No closed pull requests".to_owned(),
            },
        );
    let refresh = move || model.refresh_list();
    let show = move |index: usize| {
        model.show(if index == 0 {
            Filter::Open
        } else {
            Filter::Closed
        });
    };
    let query = model.query;
    view! {
        <List spacing=Space::None>
            <Frame padding=PADDING>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                            <Text style=TextStyle::HEADING>"Pull requests"</Text>
                            <Text style=TextStyle::CAPTION>{repository}</Text>
                        </List>
                        <Button
                            icon_only=true
                            glyph=ICON_SCAN_SEARCH
                            label="Open the inspector"
                            on_click={|| with_document(Document::open_inspector)}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_REFRESH_CW
                            label="Refresh"
                            on_click={refresh}
                        />
                    </List>
                    <MainActions model={model} />
                    <SegmentedControl
                        label="State"
                        options={view! {
                            <ChoiceOption label="Open" />
                            <ChoiceOption label="Closed" />
                        }}
                        selected={Binding::new(tab, show)}
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Text color=TextColor::Muted align=TextAlign::Center>
                            <Icon glyph=ICON_SEARCH />
                        </Text>
                        <TextInput
                            @sizing=ItemSize::Percent(100.0)
                            value={query}
                            placeholder="Filter by title, author, branch or number"
                            label="Filter pull requests"
                        />
                    </List>
                    <Show condition={loading}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <Progress width=Size::Sm label="Loading pull requests" />
                            <Text style=TextStyle::CAPTION>"Loading pull requests"</Text>
                        </List>
                    </Show>
                    <Show condition={failed}>
                        <Text color=TextColor::Danger>{error}</Text>
                    </Show>
                    <Show condition={empty}>
                        <Text style=TextStyle::CAPTION>{empty_text}</Text>
                    </Show>
                </List>
            </Frame>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding={Padding::horizontal(PADDING).bottom(PADDING)}>
                    <List spacing=Space::None>
                        <ForEach keys={visible}>|number: u64| {
                            let pull_request = model.pull_requests.get(&number);
                            let pull_request = create_memo(move || pull_request.get());
                            view! {
                                <PullRequestRow model={model} pull_request />
                            }
                        }</ForEach>
                    </List>
                </Frame>
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
    let number = pull_request.get_untracked().number;
    let selected = model.selection.memo(Some(number));
    let title = create_memo(move || pull_request.get().title);
    let details = create_memo(move || {
        let pull_request = pull_request.get();
        format!(
            "#{} by {} · updated {}",
            pull_request.number,
            pull_request.author.login,
            relative(now(), pull_request.updated)
        )
    });
    let state = create_memo(move || pull_request.get().state);
    let labels = create_memo(move || pull_request.get().labels);
    let checked_out = create_memo(move || model.current(&pull_request.get()));
    let has_tags = create_memo(move || !labels.get().is_empty() || checked_out.get());
    let modifiers = held_modifiers();
    let open = move || {
        let held = modifiers.get_untracked();
        model.select(number, held.ctrl || held.logo);
    };
    view! {
        <ListRow @test_id={format!("launcher.pull_request.{number}")} selected on_click={open}>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <StateIcon state />
                <List @sizing=ItemSize::Percent(100.0) spacing=ROW_SPACING>
                    <Text>{title}</Text>
                    <Text style=TextStyle::CAPTION>{details}</Text>
                    <Show condition={has_tags}>
                        <Labels labels={labels} checked_out={checked_out} />
                    </Show>
                </List>
            </List>
        </ListRow>
    }
}

#[component]
pub(crate) fn StateIcon(state: Memo<State>) -> NodeId {
    let glyph = create_memo(move || state_glyph(state.get()).to_owned());
    let color = create_memo(move || match state.get() {
        State::Open => TextColor::Success,
        State::Draft => TextColor::Muted,
        State::Merged => TextColor::Custom(MERGED.into()),
        State::Closed => TextColor::Danger,
    });
    view! {
        <Text color align=TextAlign::Center>
            <Icon glyph={glyph} />
        </Text>
    }
}

pub(crate) fn state_glyph(state: State) -> Glyph {
    match state {
        State::Open => ICON_SPLIT,
        State::Draft => ICON_FILE,
        State::Merged => ICON_MERGE,
        State::Closed => ICON_CIRCLE_X,
    }
}

#[component]
pub(crate) fn Labels(labels: Memo<Vec<Label>>, checked_out: Memo<bool>) -> NodeId {
    let theme = use_theme();
    let keys = create_memo(move || (0..labels.get().len()).collect::<Vec<_>>());
    view! {
        <List direction=Direction::Horizontal spacing=Space::Sm>
            <Show condition={checked_out}>
                <Chip text=CURRENT color={create_memo(move || resolve_color(theme.accent.get()))} />
            </Show>
            <ForEach keys>|index: usize| {
                let label = create_memo(move || labels.get().get(index).cloned());
                let text = create_memo(move || label.get().map(|label| label.name).unwrap_or_default());
                let color = create_memo(move || label.get().map_or(Color32::from_gray(128), |label| label.color));
                view! {
                    <Chip text color />
                }
            }</ForEach>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn Chip(text: Prop<String>, color: Prop<Color32>) -> NodeId {
    let ink = create_memo(move || TextColor::Custom((readable_on(color.get())).into()));
    view! {
        <Frame
            fill={ColorRole::custom(color)}
            radius=Radius::Full
            padding={Padding::symmetric(Space::Sm, Space::Xs2)}
        >
            <Text size=TextSize::Sm color={ink}>{text}</Text>
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
