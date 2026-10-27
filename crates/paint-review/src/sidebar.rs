use beui::Overflow;
use beui::icons::Glyph;
use beui::icons::{ICON_CIRCLE_MINUS, ICON_CIRCLE_PLUS, ICON_DIFF, ICON_IMAGE_OFF};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, TextAlign};

use crate::review::Review;
use beui::{Align, Direction, ItemSize};
use paint_review_manifest::Status;

const PADDING: Space = Space::Lg;
const GROUP_SPACING: Space = Space::Xl;
const ROW_SPACING: Space = Space::Xs2;
const HEADING_SPACING: Space = Space::Sm;

pub(crate) fn label(status: Status) -> &'static str {
    match status {
        Status::Changed => "Changed",
        Status::Added => "Added",
        Status::Removed => "Removed",
        Status::Unreadable => "Unreadable",
    }
}

pub(crate) fn glyph(status: Status) -> Glyph {
    match status {
        Status::Changed => ICON_DIFF,
        Status::Added => ICON_CIRCLE_PLUS,
        Status::Removed => ICON_CIRCLE_MINUS,
        Status::Unreadable => ICON_IMAGE_OFF,
    }
}

pub(crate) fn color(status: Status) -> TextColor {
    match status {
        Status::Changed => TextColor::Accent,
        Status::Added => TextColor::Success,
        Status::Removed => TextColor::Danger,
        Status::Unreadable => TextColor::Warning,
    }
}

pub(crate) fn role(status: Status) -> ColorRole {
    match status {
        Status::Changed => ColorRole::Accent,
        Status::Added => ColorRole::Success,
        Status::Removed => ColorRole::Danger,
        Status::Unreadable => ColorRole::Warning,
    }
}

#[component]
pub(crate) fn PaintingList(review: Review) -> NodeId {
    let summary = create_memo(move || {
        let counts: Vec<String> = Status::ALL
            .into_iter()
            .filter_map(|status| {
                let count = review.names(status).len();
                (count > 0).then(|| format!("{count} {}", label(status).to_lowercase()))
            })
            .collect();
        match counts.is_empty() {
            true => "No paintings changed".to_owned(),
            false => counts.join(", "),
        }
    });
    view! {
        <List spacing=Space::None>
            <Frame padding=PADDING>
                <List spacing=ROW_SPACING>
                    <Text style=TextStyle::HEADING>"Paintings"</Text>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.summary"}>
                        {summary}
                    </Text>
                </List>
            </Frame>
            <Separator />
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding=PADDING>
                    <List spacing=GROUP_SPACING>
                        <StatusGroup review status=Status::Changed />
                        <StatusGroup review status=Status::Added />
                        <StatusGroup review status=Status::Removed />
                        <StatusGroup review status=Status::Unreadable />
                    </List>
                </Frame>
            </Scroll>
        </List>
    }
}

#[component]
fn StatusGroup(review: Review, status: Status) -> DynamicSegment<ListChild> {
    let names = create_memo(move || review.names(status));
    let keys = create_memo(move || {
        names
            .get()
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<usize>>()
    });
    let any = create_memo(move || !names.get().is_empty());
    let heading = create_memo(move || format!("{} ({})", label(status), names.get().len()));
    let selected = review.selected;
    let here = create_selector(move || selected.get());
    view! {
        <Show condition={any}>
            <List spacing=ROW_SPACING>
                <Frame padding={Padding::symmetric(Space::Sm, ROW_SPACING)}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=HEADING_SPACING
                    >
                        <Text color={color(status)} align=TextAlign::Center>
                            <Icon glyph={glyph(status).to_owned()} />
                        </Text>
                        <Text
                            weight=Weight::Bold
                            color={color(status)}
                            @test_id={format!("paint_review.group.{}", label(status).to_lowercase())}
                        >
                            {heading}
                        </Text>
                    </List>
                </Frame>
                <ForEach keys={keys}>|index: usize| {
                    let chosen = here.memo(Some(index));
                    let name = create_memo(move || {
                        names
                            .get()
                            .into_iter()
                            .find(|(at, _)| *at == index)
                            .map(|(_, name)| name)
                            .unwrap_or_default()
                    });
                    view! {
                        <PaintingRow review index name selected={chosen} />
                    }
                }</ForEach>
            </List>
        </Show>
    }
}

#[component]
fn PaintingRow(review: Review, index: usize, name: Memo<String>, selected: Memo<bool>) -> NodeId {
    let row = format!("paint_review.painting.{}", name.get_untracked());
    view! {
        <ListRow
            selected={selected}
            @test_id={row}
            on_click={move || review.select(index)}
            on_activate={move || review.select(index)}
        >
            <Text overflow=Overflow::Ellipsis family=Family::Mono size=TextSize::Sm>{name}</Text>
        </ListRow>
    }
}
