use beui::NodeId;
use beui::icons::{ICON_EXTERNAL_LINK, ICON_GIT_COMMIT_HORIZONTAL, ICON_REFRESH_CW};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled;
use beui::unstyled::Picture;

use crate::actions::{Actions, Notes};
use crate::github::{Entry, InlineComment, Person, PullRequest, State, Tone};
use crate::markdown::Block;
use crate::model::{Loaded, Model};
use crate::time::{now, relative};
use crate::view::{Labels, MERGED, PADDING, readable_on, state_glyph};
use beui::TextAlign;

const SPACING: f32 = 12.0;
const CARD_PADDING: f32 = 12.0;
const AVATAR: f32 = 24.0;
const AVATAR_PIXELS: u32 = 64;
const BADGE_PADDING_HORIZONTAL: f32 = 10.0;
const BADGE_PADDING_VERTICAL: f32 = 4.0;
const BADGE_RADIUS: f32 = 14.0;
const QUOTE_BAR: f32 = 3.0;
const BULLET: f32 = 5.0;
const BULLET_INDENT: f32 = 8.0;
const EVENT_INDENT: f32 = 14.0;

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
    let tagged = create_memo(move || !labels.get().is_empty() || checked_out.get());
    let timeline = model.timeline(&pull_request.get_untracked());
    let loading = create_memo(move || timeline.get() == Loaded::Loading);
    let error = create_memo(move || match timeline.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    });
    let failed = create_memo(move || !error.get().is_empty());
    let entries = create_memo(move || match timeline.get() {
        Loaded::Ready(entries) => entries,
        _ => Vec::new(),
    });
    let keys = create_memo(move || (0..entries.get().len()).collect::<Vec<_>>());
    let open = move || model.open(&pull_request.get_untracked().url);
    let refresh = move || model.refresh_timeline(&pull_request.get_untracked());
    view! {
        <SelectableText child_size=ItemSize::Percent(100.0)>
            <List spacing=0.0>
                <Frame padding={Sides::symmetric(PADDING, 4.0)}>
                    <List spacing=SPACING>
                        <Text size=TextSize::Xl>{title}</Text>
                        <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                            <StateBadge state />
                            <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>
                                {summary}
                            </Text>
                        </List>
                        <Show condition={tagged}>
                            <Labels labels={labels} checked_out={checked_out} />
                        </Show>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
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
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Button
                                icon_only=true
                                glyph=ICON_REFRESH_CW
                                label="Reload the conversation"
                                on_click={refresh}
                            />
                        </List>
                        <Notes model={model} pull_request={pull_request} timeline={timeline} />
                        <Separator />
                        <Show condition={loading}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <Progress width=20.0 label="Loading the conversation" />
                                <Text style=TextStyle::CAPTION>"Loading the conversation"</Text>
                            </List>
                        </Show>
                        <Show condition={failed}>
                            <Text color=TextColor::Danger>{error}</Text>
                        </Show>
                    </List>
                </Frame>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame padding={Sides::horizontal(PADDING).top(8.0).bottom(PADDING)}>
                        <List spacing=0.0>
                            <ForEach keys>|index: usize| {
                                let entry = create_memo(move || entries.get().get(index).cloned());
                                view! {
                                    <TimelineEntry model={model} entry />
                                }
                            }</ForEach>
                        </List>
                    </Frame>
                </Scroll>
            </List>
        </SelectableText>
    }
}

#[component]
fn StateBadge(state: Memo<State>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(move || match state.get() {
        State::Open => theme.success.get(),
        State::Draft => theme.surface_raised.get(),
        State::Merged => MERGED,
        State::Closed => theme.danger.get(),
    });
    let ink = create_memo(move || TextColor::Custom(readable_on(fill.get())));
    let glyph = create_memo(move || state_glyph(state.get()).to_owned());
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
            radius={Radius::custom(BADGE_RADIUS)}
            padding={Sides::symmetric(BADGE_PADDING_HORIZONTAL, BADGE_PADDING_VERTICAL)}
        >
            <Text color={ink}>
                <Icon glyph={glyph} />
                {label}
            </Text>
        </Frame>
    }
}

#[component]
fn TimelineEntry(model: Model, entry: Memo<Option<Entry>>) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={entry}>|entry: Option<Entry>| {
                match entry {
                    Some(Entry::Comment { author, verb, tone, when, url, body, inline }) => view! {
                        <CommentCard model={model} author verb tone when url body inline />
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
    let inline_keys: Vec<usize> = (0..inline.len()).collect();
    let open = move || model.open(&url);
    view! {
        <Frame padding={Sides::vertical(6.0)}>
            <Frame
                fill=ColorRole::Surface
                border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get())).width(1.0)))}
                radius=Radius::Lg
            >
                <List spacing=0.0>
                    <Frame
                        fill=ColorRole::SurfaceRaised
                        radius=Radius::Lg
                        corners=CornerSet::TOP
                        padding={Sides::symmetric(CARD_PADDING, 8.0)}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Avatar model={model} url={author.avatar} />
                            <Text @sizing=ItemSize::Percent(100.0)>{heading}</Text>
                            <Link label={time} size=TextSize::Sm on_click={open} />
                        </List>
                    </Frame>
                    <Frame padding=CARD_PADDING>
                        <List spacing=10.0>
                            <Blocks model={model} blocks={body} />
                            <ForEach keys={inline_keys}>|index: usize| {
                                let comment = inline[index].clone();
                                view! {
                                    <InlineCommentView model={model} comment />
                                }
                            }</ForEach>
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
            border={Border::new(ColorRole::Border).width(1.0)}
            radius=Radius::Md
            padding={Sides::symmetric(10.0, 8.0)}
        >
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
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
        <Frame padding={Sides::symmetric(EVENT_INDENT, 3.0)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_GIT_COMMIT_HORIZONTAL />
                </Text>
                <Text style=TextStyle::CODE color=TextColor::Muted>{sha}</Text>
                <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>{message}</Text>
            </List>
        </Frame>
    }
}

#[component]
fn EventRow(actor: String, text: String, when: i64) -> NodeId {
    let line = format!("{actor} {text} · {}", relative(now(), when));
    view! {
        <Frame padding={Sides::symmetric(EVENT_INDENT, 3.0)}>
            <Text style=TextStyle::CAPTION>{line}</Text>
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
        <Frame width=AVATAR height=AVATAR radius={Radius::custom(12.0)} fill=ColorRole::Border>
            <Picture image={picture} radius={AVATAR / 2.0} />
        </Frame>
    }
}

#[component]
fn Blocks(model: Model, blocks: Vec<Block>) -> NodeId {
    let keys: Vec<usize> = (0..blocks.len()).collect();
    view! {
        <List spacing=8.0>
            <ForEach keys>|index: usize| {
                let block = blocks[index].clone();
                view! {
                    <BlockView model={model} block />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn BlockView(model: Model, block: Block) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={block}>|block: Block| {
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
            }</Dynamic>
        </List>
    }
}

#[component]
fn Table(model: Model, rows: Vec<Vec<Vec<Block>>>) -> NodeId {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let keys: Vec<(usize, usize)> = (0..rows.len())
        .flat_map(|row| (0..columns).map(move |column| (row, column)))
        .collect();
    view! {
        <Frame border={Border::new(ColorRole::Border).width(1.0)} radius=Radius::Md padding=8.0>
            <Grid columns={vec![Track::Fraction(1.0); columns]} column_spacing=8.0 row_spacing=8.0>
                <ForEach keys>|(row, column): (usize, usize)| {
                    let blocks = rows[row].get(column).cloned().unwrap_or_default();
                    view! {
                        <Blocks model={model} blocks />
                    }
                }</ForEach>
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
        <Frame fill=ColorRole::Background radius=Radius::Md padding={Sides::symmetric(10.0, 8.0)}>
            <Text size=TextSize::Sm family=Family::Mono>{text}</Text>
        </Frame>
    }
}

#[component]
fn Quote(text: String) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=10.0>
            <Frame @sizing=ItemSize::Fixed(QUOTE_BAR) fill=ColorRole::Border />
            <Text @sizing=ItemSize::Percent(100.0) color=TextColor::Muted>{text}</Text>
        </List>
    }
}

#[component]
fn Item(text: String) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=BULLET_INDENT>
            <Frame padding={Sides::symmetric(BULLET_INDENT, 7.0)}>
                <Frame
                    width=BULLET
                    height=BULLET
                    radius={Radius::custom(3.0)}
                    fill=ColorRole::TextMuted
                />
            </Frame>
            <Text @sizing=ItemSize::Percent(100.0)>{text}</Text>
        </List>
    }
}

#[component]
fn WebImage(model: Model, url: String, alt: String) -> NodeId {
    let image = model.image(&url);
    let picture = create_memo(move || match image.get() {
        Loaded::Ready(image) => Some(image),
        _ => None,
    });
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
    let accessibility = beui::accesskit::Node::new(beui::accesskit::Role::Button);
    view! {
        <List spacing=4.0>
            <Show condition={waiting}>|| {
                let described = described.clone(); view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Progress width=18.0 label="Loading an image" />
                        <Text style=TextStyle::CAPTION>{described}</Text>
                    </List>
                }
            }</Show>
            <Show condition={failed}>
                <Link label={missing.clone()} on_click={open.clone()} />
            </Show>
            <unstyled::Button on_click={enlarge} accessibility>
                <Picture image={picture} radius=6.0 />
            </unstyled::Button>
        </List>
    }
}
