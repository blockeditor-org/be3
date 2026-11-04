use beui::Overflow;
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled;
use beui::{Align, Align2, Direction, ImageFit, NodeId, Vec2};

use crate::Shortcuts;
use crate::images::ImageState;
use crate::review::{Review, missing};
use crate::status::{color, glyph, label};
use be_tool_paint_review_manifest::{Picture, Status};

const PADDING: Space = Space::Xl;
const TILE_SPACING: Space = Space::None;
const TILE_PADDING: Space = Space::Sm;
const THUMBNAIL_WIDTH: f32 = 144.0;
const THUMBNAIL_HEIGHT: f32 = 108.0;
const PREVIEW_MARGIN: f32 = 48.0;
const HINTS: [(&str, &str); 4] = [
    ("Hover", "see it at full size"),
    ("Hold Space", "before"),
    ("Hold Shift", "changes"),
    ("Click", "compare"),
];

#[component]
pub(crate) fn Gallery(review: Review) -> NodeId {
    let summary = create_memo(move || {
        let counts: Vec<String> = Status::ALL
            .into_iter()
            .filter_map(|status| {
                let count = review.manifest.with(|manifest| match manifest {
                    crate::Fetch::Ready(manifest) => manifest
                        .paintings
                        .iter()
                        .filter(|painting| painting.status == status)
                        .count(),
                    _ => 0,
                });
                (count > 0).then(|| format!("{count} {}", label(status).to_lowercase()))
            })
            .collect();
        match counts.is_empty() {
            true => "No paintings changed".to_owned(),
            false => format!("{}, largest changes first", counts.join(", ")),
        }
    });
    let keys = create_memo(move || (0..review.count.get()).collect::<Vec<usize>>());
    let tile_width = THUMBNAIL_WIDTH + 2.0 * TILE_PADDING.px();
    let gap = TILE_SPACING.px();
    view! {
        <List spacing=Space::None>
            <Frame padding={Padding::all(PADDING).bottom(Space::Md)}>
                <List spacing=Space::Xs2>
                    <Text style=TextStyle::HEADING>"Paintings"</Text>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.summary"}>
                        {summary}
                    </Text>
                </List>
            </Frame>
            <Shortcuts hints={HINTS.to_vec()} />
            <Separator />
            <ListItem grow=1.0>
                <Container>|size: ReadSignal<Vec2>| {
                    create_effect(move || {
                        let inner = size.get().x - 2.0 * PADDING.px();
                        let columns = ((inner + gap) / (tile_width + gap)).floor().max(1.0);
                        review.columns.set(columns as usize);
                    });
                    view! {
                        <Layers>
                            <Scroll>
                                <Frame padding=PADDING>
                                    <List
                                        direction=Direction::Horizontal
                                        spacing=TILE_SPACING
                                        wrap=true
                                        @test_id={"paint_review.grid"}
                                    >
                                        <ForEach keys={keys}>|index: usize|
                                            <Tile review index />
                                        </ForEach>
                                    </List>
                                </Frame>
                            </Scroll>
                            <Preview review size />
                        </Layers>
                    }
                }</Container>
            </ListItem>
        </List>
    }
}

#[component]
fn Tile(review: Review, index: usize) -> NodeId {
    let (name, status) = untrack(|| {
        review
            .with_painting(index, |painting| (painting.name.clone(), painting.status))
            .unwrap_or((String::new(), Status::Unreadable))
    });
    let shown = create_memo(move || {
        let side = review.side.get();
        let picture = review
            .with_painting(index, |painting| {
                painting
                    .frames
                    .first()
                    .and_then(|frame| side.of(frame))
                    .cloned()
            })
            .flatten();
        (side, picture)
    });
    let image = create_memo(move || {
        let (_, picture) = shown.get();
        match review.images.state(&picture?.thumbnail).get() {
            ImageState::Ready(image) => Some(image),
            _ => None,
        }
    });
    let notice = create_memo(move || {
        let (side, picture) = shown.get();
        match picture {
            Some(_) => String::new(),
            None => missing(status, side).to_owned(),
        }
    });
    let noticed = create_memo(move || !notice.get().is_empty());
    let tile_id = format!("paint_review.painting.{name}");
    let label = name.clone();
    view! {
        <unstyled::Button
            label={label}
            icon_only=true
            on_click={move || review.select(index)}
            @test_id={tile_id}
        >|handle: unstyled::ButtonHandle| {
            let hovered = handle.hovered;
            create_effect(move || {
                if hovered.get() {
                    review.hovered.set(Some(index));
                } else if review.hovered.get_untracked() == Some(index) {
                    review.hovered.set(None);
                }
            });
            on_cleanup(move || {
                if review.hovered.get_untracked() == Some(index) {
                    review.hovered.set(None);
                }
            });
            let keyboard = focus_ring(handle.focused);
            let fill = create_memo(move || match hovered.get() {
                true => ColorRole::Hover,
                false => ColorRole::None,
            });
            let border = create_memo(move || match hovered.get() || keyboard.get() {
                true => Some(Border::new(ColorRole::Accent).width(Stroke::Focus)),
                false => None,
            });
            view! {
                <Frame padding=TILE_PADDING fill={fill} border={border}>
                    <List spacing=Space::Xs>
                        <Frame
                            width={Size::Custom(THUMBNAIL_WIDTH)}
                            height={Size::Custom(THUMBNAIL_HEIGHT)}
                            fill=ColorRole::Surface
                            radius=Radius::Sm
                            align=Align::Center
                        >
                            <Layers>
                                <Picture image={image} fit=ImageFit::ScaleDown />
                                <Show condition={noticed}>
                                    <Frame align=Align::Center>
                                        <Text
                                            style=TextStyle::CAPTION
                                            color={color(status)}
                                            align=beui::TextAlign::Center
                                        >
                                            {notice}
                                        </Text>
                                    </Frame>
                                </Show>
                            </Layers>
                        </Frame>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                            <Text size=TextSize::Sm color={color(status)}>
                                <Icon glyph={glyph(status).to_owned()} />
                            </Text>
                            <ListItem length=THUMBNAIL_WIDTH shrink=1.0>
                                <Text
                                    overflow=Overflow::Ellipsis
                                    family=Family::Mono
                                    size=TextSize::Sm
                                >
                                    {name.clone()}
                                </Text>
                            </ListItem>
                        </List>
                    </List>
                </Frame>
            }
        }</unstyled::Button>
    }
}

#[component]
fn Preview(review: Review, size: ReadSignal<Vec2>) -> NodeId {
    let shown = create_memo(move || {
        let index = review.hovered.get()?;
        let side = review.side.get();
        review.with_painting(index, |painting| {
            let picture = painting
                .frames
                .first()
                .and_then(|frame| side.of(frame))
                .cloned();
            (painting.name.clone(), painting.status, side, picture)
        })
    });
    let showing = create_memo(move || shown.with(Option::is_some));
    let picture = create_memo(move || shown.get().and_then(|(_, _, _, picture)| picture));
    let image = create_memo(move || {
        let picture = picture.get()?;
        match review.images.state(&picture.image).get() {
            ImageState::Ready(image) => Some(image),
            _ => match review.images.state(&picture.thumbnail).get() {
                ImageState::Ready(image) => Some(image),
                _ => None,
            },
        }
    });
    let pictured = create_memo(move || picture.with(Option::is_some));
    let fitted = create_memo(move || {
        let picture = picture.get()?;
        Some(fitted(&picture, size.get()))
    });
    let width = create_memo(move || Size::Custom(fitted.get().map_or(0.0, |size| size.x)));
    let height = create_memo(move || Size::Custom(fitted.get().map_or(0.0, |size| size.y)));
    let caption = create_memo(move || match shown.get() {
        Some((name, status, side, picture)) => match picture {
            Some(picture) => format!(
                "{name}: {}, {} × {}",
                side.label().to_lowercase(),
                picture.width,
                picture.height
            ),
            None => format!("{name}: {}", missing(status, side).to_lowercase()),
        },
        None => String::new(),
    });
    view! {
        <Frame align={Align2::CENTER}>
            <List spacing=Space::None>
                <Show condition={showing}>
                    <Frame
                        fill=ColorRole::Surface
                        border={Some(Border::DEFAULT)}
                        radius=Radius::Md
                        padding=Space::Sm
                        @test_id={"paint_review.preview"}
                    >
                        <List spacing=Space::Xs align=Align::Center>
                            <Show condition={pictured}>
                                <Frame width={width} height={height}>
                                    <Picture image={image} fit=ImageFit::Fill smooth=false />
                                </Frame>
                            </Show>
                            <Text
                                style=TextStyle::CAPTION
                                @test_id={"paint_review.preview.caption"}
                            >
                                {caption}
                            </Text>
                        </List>
                    </Frame>
                </Show>
            </List>
        </Frame>
    }
}

fn fitted(picture: &Picture, room: Vec2) -> Vec2 {
    let actual = Vec2::new(picture.width as f32, picture.height as f32);
    let room = (room - Vec2::splat(PREVIEW_MARGIN)).max(Vec2::splat(1.0));
    let scale = (room.x / actual.x).min(room.y / actual.y).min(1.0);
    actual * scale
}
