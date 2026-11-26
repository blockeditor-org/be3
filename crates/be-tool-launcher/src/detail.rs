use beui::NodeId;
use beui::icons::{ICON_EXTERNAL_LINK, ICON_GIT_COMMIT_HORIZONTAL, ICON_REFRESH_CW};
use beui::reactive::*;
use beui::styled::*;

use crate::actions::{Actions, Notes};
use crate::github::{Entry, InlineComment, Person, PullRequest, State, Tone};
use crate::markdown::Block;
use crate::model::{Loaded, Model};
use crate::time::{now, relative};
use crate::view::{Labels, MERGED, PADDING, readable_on, state_glyph};
use beui::TextAlign;
use beui::{Align, Direction, Track};

const SPACING: Space = Space::Lg;
const CARD_PADDING: Space = Space::Lg;
const AVATAR: Size = Size::Md;
const AVATAR_PIXELS: u32 = 64;
const QUOTE_BAR: f32 = 3.0;
const BULLET: Size = Size::Custom(5.0);
const BULLET_INDENT: Space = Space::Md;
const EVENT_INDENT: Space = Space::Lg;

#[derive(Clone, Copy)]
struct Shown(u64);

#[component]
pub(crate) fn Detail(model: Model, pull_request: Memo<PullRequest>) -> NodeId {
    provide_context(Shown(
        pull_request.with_untracked(|pull_request| pull_request.number),
    ));
    view! {
        <PullRequestView model={model} pull_request={pull_request} />
    }
}

#[cfg(target_os = "android")]
pub(crate) fn placeholder(number: u64) -> PullRequest {
    PullRequest {
        number,
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
    let title = create_memo(move || {
        let pull_request = pull_request.get();
        format!("{} #{}", pull_request.title, pull_request.number)
    });
    let state = create_memo(move || pull_request.get().state);
    let summary = create_memo(move || {
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
    });
    let labels = create_memo(move || pull_request.get().labels);
    let checked_out = create_memo(move || model.current(&pull_request.get()));
    let timeline = model.timeline(&pull_request.get_untracked());
    let loading = create_memo(move || timeline.get() == Loaded::Loading);
    let error = create_memo(move || match timeline.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    });
    let entries = create_memo(move || match timeline.get() {
        Loaded::Ready(entries) => entries,
        _ => Vec::new(),
    });
    let keys = create_memo(move || (0..entries.get().len()).collect::<Vec<_>>());
    let open = move || model.open(&pull_request.get_untracked().url);
    let refresh = move || model.refresh_timeline(&pull_request.get_untracked());
    view! {
        <SelectableText child_grow=1.0>
            <List spacing=Space::None>
                <Frame padding={Padding::symmetric(PADDING, Space::Xs)}>
                    <List spacing=SPACING>
                        <Text size=TextSize::Xl>{title}</Text>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <StateBadge state />
                            <ListItem grow=1.0>
                                <Text style=TextStyle::CAPTION>{summary}</Text>
                            </ListItem>
                        </List>
                        <Show
                            value={move || labels.with(|labels| !labels.is_empty()) || checked_out.get()}
                        >
                            <Labels labels={labels} checked_out={checked_out} />
                        </Show>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <Actions
                                model={model}
                                pull_request={pull_request}
                                timeline={timeline}
                            />
                            <Button
                                label="Open on GitHub"
                                glyph=ICON_EXTERNAL_LINK
                                variant=ButtonVariant::Ghost
                                on_click={open}
                            />
                            <Spacer />
                            <Button
                                icon_only=true
                                glyph=ICON_REFRESH_CW
                                label="Reload the conversation"
                                on_click={refresh}
                            />
                        </List>
                        <Notes model={model} pull_request={pull_request} timeline={timeline} />
                        <Separator />
                        <Show value={loading}>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <Progress width=Size::Sm label="Loading the conversation" />
                                <Text style=TextStyle::CAPTION>"Loading the conversation"</Text>
                            </List>
                        </Show>
                        <Show value={move || !error.with(String::is_empty)}>
                            <Text color=TextColor::Danger>{error}</Text>
                        </Show>
                    </List>
                </Frame>
                <ListItem grow=1.0>
                    <Scroll>
                        <Frame
                            padding={Padding::horizontal(PADDING).top(Space::Md).bottom(PADDING)}
                        >
                            <List spacing=Space::None>
                                <ForEach value={keys}>|index: usize| {
                                    let entry = create_memo(move || entries.get().get(index).cloned());
                                    view! {
                                        <TimelineEntry model={model} entry />
                                    }
                                }</ForEach>
                            </List>
                        </Frame>
                    </Scroll>
                </ListItem>
            </List>
        </SelectableText>
    }
}

#[component]
fn StateBadge(state: Memo<State>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || match state.get() {
        State::Open => theme.success.get(),
        State::Draft => theme.control.get(),
        State::Merged => MERGED.into(),
        State::Closed => theme.danger.get(),
    });
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
            fill={ColorRole::custom(fill)}
            radius=Radius::Full
            padding={Padding::symmetric(Space::Md, Space::Xs)}
        >
            <Text color={move || TextColor::Custom(readable_on(resolve_color(fill.get())).into())}>
                <Icon glyph={move || state_glyph(state.get()).to_owned()} />
                {label}
            </Text>
        </Frame>
    }
}

#[component]
fn TimelineEntry(model: Model, entry: Memo<Option<Entry>>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Dynamic
                value={entry}
                fallback=()
            >|entry: Entry| {
                match entry {
                    Entry::Comment { author, verb, tone, when, url, body, inline } => view! {
                        <CommentCard model={model} author verb tone when url body inline />
                    },
                    Entry::Commit { sha, message } => view! {
                        <CommitRow sha message />
                    },
                    Entry::Event { actor, text, when } => view! {
                        <EventRow actor text when />
                    },
                }
            }</Dynamic>
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
    let outline = create_memo(move || match tone {
        Tone::Neutral => theme.border.get(),
        Tone::Approved => theme.success.get(),
        Tone::ChangesRequested => theme.warning.get(),
    });
    let heading = format!("{} {verb}", author.login);
    let time = relative(now(), when);
    let open = move || model.open(&url);
    view! {
        <Frame padding={Padding::vertical(Space::Sm)}>
            <Frame
                fill=ColorRole::Surface
                border={move || Border::new(ColorRole::Custom(outline.get())).width(Stroke::Hairline)}
                radius=Radius::Lg
            >
                <List spacing=Space::None>
                    <Frame
                        fill=ColorRole::Shade
                        radius=Radius::Lg
                        corners=CornerSet::TOP
                        padding={Padding::symmetric(CARD_PADDING, Space::Md)}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <Avatar model={model} url={author.avatar} />
                            <ListItem grow=1.0>
                                <Text>{heading}</Text>
                            </ListItem>
                            <Link label={time} size=TextSize::Sm on_click={open} />
                        </List>
                    </Frame>
                    <Frame padding=CARD_PADDING>
                        <List spacing=Space::Md>
                            <Blocks model={model} blocks={body} />
                            <ForEach value={inline}>|comment: InlineComment|
                                <InlineCommentView model={model} comment />
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
    let InlineComment {
        author,
        location,
        body,
    } = comment;
    view! {
        <Frame
            fill=ColorRole::Background
            border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
            radius=Radius::Md
            padding={Padding::symmetric(Space::Md, Space::Md)}
        >
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::CODE>{location}</Text>
                    <Text style=TextStyle::CAPTION>{author}</Text>
                </List>
                <Blocks model blocks={body} />
            </List>
        </Frame>
    }
}

#[component]
fn CommitRow(sha: String, message: String) -> NodeId {
    view! {
        <Frame padding={Padding::symmetric(EVENT_INDENT, Space::Xs2)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_GIT_COMMIT_HORIZONTAL />
                </Text>
                <Text style=TextStyle::CODE color=TextColor::Muted>{sha}</Text>
                <ListItem grow=1.0>
                    <Text style=TextStyle::CAPTION>{message}</Text>
                </ListItem>
            </List>
        </Frame>
    }
}

#[component]
fn EventRow(actor: String, text: String, when: i64) -> NodeId {
    let line = format!("{actor} {text} · {}", relative(now(), when));
    view! {
        <Frame padding={Padding::symmetric(EVENT_INDENT, Space::Xs2)}>
            <Text style=TextStyle::CAPTION>{line}</Text>
        </Frame>
    }
}

#[component]
fn Avatar(model: Model, url: String) -> NodeId {
    let separator = if url.contains('?') { '&' } else { '?' };
    let image = model.image(&format!("{url}{separator}s={AVATAR_PIXELS}"));
    view! {
        <Frame width=AVATAR height=AVATAR radius=Radius::Full fill=ColorRole::Border>
            <Picture
                image={move || match image.get() {
                    Loaded::Ready(image) => Some(image),
                    _ => None,
                }}
                radius=Radius::Full
            />
        </Frame>
    }
}

#[component]
fn Blocks(model: Model, blocks: Vec<Block>) -> NodeId {
    view! {
        <List spacing=Space::Md>
            <ForEach value={blocks}>|block: Block|
                <BlockView model={model} block />
            </ForEach>
        </List>
    }
}

#[component]
fn BlockView(model: Model, block: Block) -> NodeId {
    match block {
        Block::Heading(text) => view! {
            <Text size=TextSize::Lg weight=Weight::Bold>{text}</Text>
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
            <WebImage model={model} url alt />
        },
        Block::Table(rows) => view! {
            <Table model={model} rows />
        },
        Block::Rule => view! {
            <Separator />
        },
    }
}

#[component]
fn Table(model: Model, rows: Vec<Vec<Vec<Block>>>) -> NodeId {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let cells: Vec<Vec<Block>> = rows
        .iter()
        .flat_map(|row| {
            (0..columns).map(move |column| row.get(column).cloned().unwrap_or_default())
        })
        .collect();
    view! {
        <Frame
            border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
            radius=Radius::Md
            padding=Space::Md
        >
            <Grid
                columns={vec![Track::Fraction(1.0); columns]}
                column_spacing=Space::Md
                row_spacing=Space::Md
            >
                <ForEach value={cells}>|blocks: Vec<Block>|
                    <Blocks model={model} blocks />
                </ForEach>
            </Grid>
        </Frame>
    }
}

#[component]
fn Prose(text: String) -> NodeId {
    view! {
        <Text>{text}</Text>
    }
}

#[component]
fn CodeBlock(text: String) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::Background
            radius=Radius::Md
            padding={Padding::symmetric(Space::Md, Space::Md)}
        >
            <Text size=TextSize::Sm family=Family::Mono>{text}</Text>
        </Frame>
    }
}

#[component]
fn Quote(text: String) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=Space::Md>
            <ListItem length=QUOTE_BAR>
                <Frame fill=ColorRole::Border />
            </ListItem>
            <ListItem grow=1.0>
                <Text color=TextColor::Muted>{text}</Text>
            </ListItem>
        </List>
    }
}

#[component]
fn Item(text: String) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=BULLET_INDENT>
            <Frame padding={Padding::symmetric(BULLET_INDENT, Space::Sm)}>
                <Frame width=BULLET height=BULLET radius=Radius::Full fill=ColorRole::TextMuted />
            </Frame>
            <ListItem grow=1.0>
                <Text>{text}</Text>
            </ListItem>
        </List>
    }
}

#[component]
fn WebImage(model: Model, url: String, alt: String) -> NodeId {
    let image = model.image(&url);
    let waiting = create_memo(move || image.get() == Loaded::Loading);
    let failed = create_memo(move || matches!(image.get(), Loaded::Failed(_)));
    let described = if alt.is_empty() {
        "image".to_owned()
    } else {
        alt
    };
    let missing = format!("Could not load {described}");
    let open = {
        let url = url.clone();
        move || model.open(&url)
    };
    let shown = use_context::<Shown>();
    let enlarge = move || {
        if let Some(Shown(number)) = shown {
            model.view_image(number, &url);
        }
    };
    let mut accessibility = beui::accesskit::Node::new(beui::accesskit::Role::Button);
    accessibility.set_label(format!("Enlarge {described}"));
    view! {
        <List spacing=Space::Xs>
            <Show value={waiting}>|| {
                let described = described.clone(); view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Progress width=Size::Sm label="Loading an image" />
                        <Text style=TextStyle::CAPTION>{described}</Text>
                    </List>
                }
            }</Show>
            <Show value={failed}>
                <Link label={missing.clone()} on_click={open.clone()} />
            </Show>
            <Pressable on_click={enlarge} accessibility>
                <Picture
                    image={move || match image.get() {
                        Loaded::Ready(image) => Some(image),
                        _ => None,
                    }}
                    radius=Radius::Md
                />
            </Pressable>
        </List>
    }
}
