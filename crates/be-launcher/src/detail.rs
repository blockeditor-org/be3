use beui::NodeId;
use beui::icons::{ICON_COMMIT, ICON_OPEN_IN_NEW, ICON_REFRESH};
use beui::reactive::{
    Align, Direction, Dynamic, ForEach, Frame, ItemSize, Keyed, List, Memo, Picture, ReadSignal,
    Show, Spacer, Text, clone, component, create_memo, view,
};
use beui::styled::theme::{CARD_RADIUS, FONT_BODY, FONT_HEADING, FONT_SMALL, FONT_TITLE};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, Code, Icon, IconButton, Link, Scroll, Separator, Spinner,
    use_theme,
};
use beui::unstyled;

use crate::github::{Entry, InlineComment, Person, PullRequest, State, Tone};
use crate::markdown::Block;
use crate::model::{Loaded, Model};
#[cfg(target_os = "android")]
use crate::phone::{Actions, Notes};
use crate::time::{now, relative};
use crate::view::{Labels, MERGED, PADDING, readable_on, state_glyph};
#[cfg(not(target_os = "android"))]
use crate::workspace::{Actions, Notes};

const SPACING: f32 = 12.0;
const CARD_PADDING: f32 = 12.0;
const AVATAR: f32 = 24.0;
const AVATAR_PIXELS: u32 = 64;
const BADGE_PADDING_HORIZONTAL: f32 = 10.0;
const BADGE_PADDING_VERTICAL: f32 = 4.0;
const BADGE_RADIUS: u8 = 14;
const QUOTE_BAR: f32 = 3.0;
const BULLET: f32 = 5.0;
const BULLET_INDENT: f32 = 8.0;
const EVENT_INDENT: f32 = 14.0;

#[component]
pub(crate) fn Detail(model: Model) -> NodeId {
    let none = create_memo(clone!(model -> move || model.selected.with(Option::is_none)));
    let selected = model.selected.clone();
    view! {
        <List spacing=0.0>
            <Show condition={none}>
                <Frame padding_horizontal=PADDING padding_vertical=4.0>
                    <List spacing=8.0>
                        <Text
                            string="Pick a pull request"
                            font_size=FONT_TITLE
                            color={use_theme().text.clone()}
                        />
                        <Caption
                            content="Its conversation shows here, and you can check it out and run it."
                            wrap=true
                        />
                    </List>
                </Frame>
            </Show>
            <Keyed
                value={selected}
                key={|pull_request: Option<PullRequest>| pull_request.map(|pull_request| pull_request.number)}
            >
                {move |pull_request: ReadSignal<Option<PullRequest>>| view! {
                    <Selected @sizing=ItemSize::Percent(100.0) model={model.clone()} pull_request />
                }}
            </Keyed>
        </List>
    }
}

#[component]
fn Selected(model: Model, pull_request: ReadSignal<Option<PullRequest>>) -> NodeId {
    let shown = create_memo(clone!(pull_request -> move || pull_request.with(Option::is_some)));
    let pull_request = create_memo(move || pull_request.get());
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <PullRequestView
                    @sizing=ItemSize::Percent(100.0)
                    model
                    pull_request={create_memo(move || pull_request.get().unwrap_or_else(placeholder))}
                />
            </Show>
        </List>
    }
}

fn placeholder() -> PullRequest {
    PullRequest {
        number: 0,
        title: String::new(),
        state: State::Open,
        author: Person {
            login: String::new(),
            avatar: String::new(),
        },
        branch: String::new(),
        head_sha: String::new(),
        same_repository: true,
        labels: Vec::new(),
        created: 0,
        updated: 0,
        url: String::new(),
        body: String::new(),
    }
}

#[component]
fn PullRequestView(model: Model, pull_request: Memo<PullRequest>) -> NodeId {
    let theme = use_theme();
    let title = create_memo(clone!(pull_request -> move || {
        let pull_request = pull_request.get();
        format!("{} #{}", pull_request.title, pull_request.number)
    }));
    let state = create_memo(clone!(pull_request -> move || pull_request.get().state));
    let summary = create_memo(clone!(pull_request -> move || {
        let pull_request = pull_request.get();
        let verb = match pull_request.state {
            State::Merged => "merged",
            State::Closed => "wanted to merge",
            State::Open | State::Draft => "wants to merge",
        };
        format!(
            "{} {verb} {} · opened {}",
            pull_request.author.login,
            pull_request.branch,
            relative(now(), pull_request.created)
        )
    }));
    let labels = create_memo(clone!(pull_request -> move || pull_request.get().labels));
    let checked_out = create_memo(clone!(model pull_request -> move || {
        model.current(&pull_request.get())
    }));
    let tagged = create_memo(clone!(labels checked_out -> move || {
        !labels.get().is_empty() || checked_out.get()
    }));
    let timeline = model.timeline(&pull_request.get_untracked());
    let loading = create_memo(clone!(timeline -> move || timeline.get() == Loaded::Loading));
    let error = create_memo(clone!(timeline -> move || match timeline.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    }));
    let failed = create_memo(clone!(error -> move || !error.get().is_empty()));
    let entries = create_memo(clone!(timeline -> move || match timeline.get() {
        Loaded::Ready(entries) => entries,
        _ => Vec::new(),
    }));
    let keys = create_memo(clone!(entries -> move || (0..entries.get().len()).collect::<Vec<_>>()));
    let actions = model.clone();
    let notes = model.clone();
    let noted = pull_request.clone();
    let notes_timeline = timeline.clone();
    let open = clone!(model pull_request -> move || model.open(&pull_request.get_untracked().url));
    let refresh =
        clone!(model pull_request -> move || model.refresh_timeline(&pull_request.get_untracked()));
    view! {
        <List spacing=0.0>
            <Frame padding_horizontal=PADDING padding_vertical=4.0>
                <List spacing=SPACING>
                    <Text
                        string={title}
                        font_size=FONT_TITLE
                        color={theme.text.clone()}
                        wrap=true
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                        <StateBadge state />
                        <Caption @sizing=ItemSize::Percent(100.0) content={summary} wrap=true />
                    </List>
                    <Show condition={tagged}>
                        <Labels labels checked_out />
                    </Show>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Actions
                            model={actions.clone()}
                            pull_request={pull_request.clone()}
                            timeline={timeline.clone()}
                        />
                        <Button
                            label="Open on GitHub"
                            glyph=ICON_OPEN_IN_NEW
                            variant=ButtonVariant::Ghost
                            on_click={open}
                        />
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <IconButton
                            glyph=ICON_REFRESH
                            label="Reload the conversation"
                            on_click={refresh}
                        />
                    </List>
                    <Notes
                        model={notes.clone()}
                        pull_request={noted.clone()}
                        timeline={notes_timeline.clone()}
                    />
                    <Separator />
                    <Show condition={loading}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner width=20.0 label="Loading the conversation" />
                            <Caption content="Loading the conversation" />
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
                <Frame padding_horizontal=PADDING>
                    <List spacing=0.0>
                        <Spacer @sizing=ItemSize::Fixed(8.0) />
                        <ForEach keys>
                            {move |index: usize| {
                                let entry = create_memo(clone!(entries -> move || entries.get().get(index).cloned()));
                                view! {
                                    <TimelineEntry model={model.clone()} entry />
                                }
                            }}
                        </ForEach>
                        <Spacer @sizing=ItemSize::Fixed(PADDING) />
                    </List>
                </Frame>
            </Scroll>
        </List>
    }
}

#[component]
fn StateBadge(state: Memo<State>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(state -> move || match state.get() {
        State::Open => theme.success.get(),
        State::Draft => theme.surface_raised.get(),
        State::Merged => MERGED,
        State::Closed => theme.danger.get(),
    }));
    let ink = create_memo(clone!(fill -> move || readable_on(fill.get())));
    let glyph = create_memo(clone!(state -> move || state_glyph(state.get()).to_owned()));
    let label = create_memo(move || {
        match state.get() {
            State::Open => "Open",
            State::Draft => "Draft",
            State::Merged => "Merged",
            State::Closed => "Closed",
        }
        .to_owned()
    });
    view! {
        <Frame
            color={fill}
            radius=BADGE_RADIUS
            padding_horizontal=BADGE_PADDING_HORIZONTAL
            padding_vertical=BADGE_PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Icon glyph color={ink.clone()} />
                <Text string={label} font_size=FONT_BODY color={ink} />
            </List>
        </Frame>
    }
}

#[component]
fn TimelineEntry(model: Model, entry: Memo<Option<Entry>>) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={entry}>
                {move |entry: Option<Entry>| match entry {
                    Some(Entry::Comment { author, verb, tone, when, url, body, inline }) => view! {
                        <CommentCard model={model.clone()} author verb tone when url body inline />
                    },
                    Some(Entry::Commit { sha, message }) => view! {
                        <CommitRow sha message />
                    },
                    Some(Entry::Event { actor, text, when }) => view! {
                        <EventRow actor text when />
                    },
                    None => view! {
                        <Spacer />
                    },
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn CommentCard(
    model: Model,
    author: Person,
    verb: String,
    tone: Tone,
    when: i64,
    url: String,
    body: Vec<Block>,
    inline: Vec<InlineComment>,
) -> NodeId {
    let theme = use_theme();
    let outline = create_memo(clone!(theme -> move || match tone {
        Tone::Neutral => theme.border.get(),
        Tone::Approved => theme.success.get(),
        Tone::ChangesRequested => theme.warning.get(),
    }));
    let heading = format!("{} {verb}", author.login);
    let time = relative(now(), when);
    let inline_keys: Vec<usize> = (0..inline.len()).collect();
    let open = clone!(model -> move || model.open(&url));
    let avatar = model.clone();
    let body_model = model.clone();
    view! {
        <Frame padding_vertical=6.0>
            <Frame
                color={theme.surface.clone()}
                outline
                outline_width=1.0
                outline_visible=true
                radius=CARD_RADIUS
            >
                <List spacing=0.0>
                    <Frame
                        color={theme.surface_raised.clone()}
                        radius=CARD_RADIUS
                        padding_horizontal=CARD_PADDING
                        padding_vertical=8.0
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Avatar model={avatar} url={author.avatar} />
                            <Body @sizing=ItemSize::Percent(100.0) content={heading} />
                            <Link label={time} font_size=FONT_SMALL on_click={open} />
                        </List>
                    </Frame>
                    <Frame padding_horizontal=CARD_PADDING padding_vertical=CARD_PADDING>
                        <List spacing=10.0>
                            <Blocks model={body_model} blocks={body} />
                            <ForEach keys={inline_keys}>
                                {move |index: usize| {
                                    let comment = inline[index].clone();
                                    view! {
                                        <InlineCommentView model={model.clone()} comment />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Frame>
                </List>
            </Frame>
        </Frame>
    }
}

#[component]
fn InlineCommentView(model: Model, comment: InlineComment) -> NodeId {
    let theme = use_theme();
    let InlineComment {
        author,
        location,
        body,
    } = comment;
    view! {
        <Frame
            color={theme.background.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=6
            padding_horizontal=10.0
            padding_vertical=8.0
        >
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Code content={location} />
                    <Caption content={author} />
                </List>
                <Blocks model blocks={body} />
            </List>
        </Frame>
    }
}

#[component]
fn CommitRow(sha: String, message: String) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=EVENT_INDENT padding_vertical=3.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Icon glyph=ICON_COMMIT color={theme.text_muted.clone()} />
                <Code content={sha} color={theme.text_muted.clone()} />
                <Caption @sizing=ItemSize::Percent(100.0) content={message} />
            </List>
        </Frame>
    }
}

#[component]
fn EventRow(actor: String, text: String, when: i64) -> NodeId {
    let line = format!("{actor} {text} · {}", relative(now(), when));
    view! {
        <Frame padding_horizontal=EVENT_INDENT padding_vertical=3.0>
            <Caption content={line} wrap=true />
        </Frame>
    }
}

#[component]
fn Avatar(model: Model, url: String) -> NodeId {
    let separator = if url.contains('?') { '&' } else { '?' };
    let image = model.image(&format!("{url}{separator}s={AVATAR_PIXELS}"));
    let picture = create_memo(move || match image.get() {
        Loaded::Ready(image) => Some(image),
        _ => None,
    });
    view! {
        <Frame width=AVATAR height=AVATAR radius=12 color={use_theme().border.clone()}>
            <Picture image={picture} radius={AVATAR / 2.0} />
        </Frame>
    }
}

#[component]
fn Blocks(model: Model, blocks: Vec<Block>) -> NodeId {
    let keys: Vec<usize> = (0..blocks.len()).collect();
    view! {
        <List spacing=8.0>
            <ForEach keys>
                {move |index: usize| {
                    let block = blocks[index].clone();
                    view! {
                        <BlockView model={model.clone()} block />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn BlockView(model: Model, block: Block) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={block}>
                {move |block: Block| match block {
                    Block::Heading(text) => view! {
                        <Text
                            string={text}
                            font_size=FONT_HEADING
                            color={use_theme().text.clone()}
                            bold=true
                            wrap=true
                        />
                    },
                    Block::Paragraph(text) => view! {
                        <Prose text />
                    },
                    Block::Code(text) => view! {
                        <CodeBlock text />
                    },
                    Block::Quote(text) => view! {
                        <Quote text />
                    },
                    Block::Item(text) => view! {
                        <Item text />
                    },
                    Block::Image { url, alt } => view! {
                        <WebImage model={model.clone()} url alt />
                    },
                    Block::Table(rows) => view! {
                        <Table model={model.clone()} rows />
                    },
                    Block::Rule => view! {
                        <Separator />
                    },
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn Table(model: Model, rows: Vec<Vec<Vec<Block>>>) -> NodeId {
    let theme = use_theme();
    let keys: Vec<usize> = (0..rows.len()).collect();
    view! {
        <Frame
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=6
            padding_horizontal=8.0
            padding_vertical=8.0
        >
            <List spacing=8.0>
                <ForEach keys>
                    {move |index: usize| {
                        let cells = rows[index].clone();
                        view! {
                            <TableRow model={model.clone()} cells />
                        }
                    }}
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn TableRow(model: Model, cells: Vec<Vec<Block>>) -> NodeId {
    let share = 100.0 / cells.len().max(1) as f32;
    let keys: Vec<usize> = (0..cells.len()).collect();
    view! {
        <List direction=Direction::Horizontal spacing=8.0>
            <ForEach keys>
                {move |index: usize| {
                    let blocks = cells[index].clone();
                    view! {
                        <Blocks @sizing=ItemSize::Percent(share) model={model.clone()} blocks />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn Prose(text: String) -> NodeId {
    view! {
        <Text string={text} font_size=FONT_BODY color={use_theme().text.clone()} wrap=true />
    }
}

#[component]
fn CodeBlock(text: String) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            radius=6
            padding_horizontal=10.0
            padding_vertical=8.0
        >
            <Text
                string={text}
                font_size=FONT_SMALL
                color={theme.text.clone()}
                monospace=true
                wrap=true
            />
        </Frame>
    }
}

#[component]
fn Quote(text: String) -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=10.0>
            <Frame @sizing=ItemSize::Fixed(QUOTE_BAR) color={theme.border.clone()} />
            <Text
                @sizing=ItemSize::Percent(100.0)
                string={text}
                font_size=FONT_BODY
                color={theme.text_muted.clone()}
                wrap=true
            />
        </List>
    }
}

#[component]
fn Item(text: String) -> NodeId {
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal spacing=BULLET_INDENT>
            <Frame padding_horizontal=BULLET_INDENT padding_vertical=7.0>
                <Frame width=BULLET height=BULLET radius=3 color={theme.text_muted.clone()} />
            </Frame>
            <Text
                @sizing=ItemSize::Percent(100.0)
                string={text}
                font_size=FONT_BODY
                color={theme.text.clone()}
                wrap=true
            />
        </List>
    }
}

#[component]
fn WebImage(model: Model, url: String, alt: String) -> NodeId {
    let image = model.image(&url);
    let picture = create_memo(clone!(image -> move || match image.get() {
        Loaded::Ready(image) => Some(image),
        _ => None,
    }));
    let waiting = create_memo(clone!(image -> move || image.get() == Loaded::Loading));
    let failed = create_memo(clone!(image -> move || matches!(image.get(), Loaded::Failed(_))));
    let described = if alt.is_empty() {
        "image".to_owned()
    } else {
        alt
    };
    let missing = format!("Could not load {described}");
    let open = clone!(model url -> move || model.open(&url));
    let enlarge = move || model.view_image(&url);
    let accessibility = beui::accesskit::Node::new(beui::accesskit::Role::Button);
    view! {
        <List spacing=4.0>
            <Show condition={waiting}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner width=18.0 label="Loading an image" />
                    <Caption content={described} />
                </List>
            </Show>
            <Show condition={failed}>
                <Link label={missing} on_click={open} />
            </Show>
            <unstyled::Button on_click={enlarge} accessibility>
                <Picture image={picture} radius=6.0 />
            </unstyled::Button>
        </List>
    }
}
