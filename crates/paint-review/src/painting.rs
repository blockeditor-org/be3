use beui::Overflow;
use beui::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, TextAlign};

use crate::review::{Mode, Review};
use crate::sidebar::{color, glyph, label, role};
use crate::stage::Stage;
use paint_review_manifest::Status;

const PADDING: Space = Space::Xl;
const SPACING: Space = Space::Md;
const HINT_SPACING: Space = Space::Xs;
const BADGE_PADDING: Padding = Padding::symmetric(Space::Md, Space::Xs2);
const SHORTCUTS: [(&str, &str); 4] = [
    ("Space", "flip before and after"),
    ("1 2 3 4", "choose the view"),
    ("J K", "next, previous painting"),
    (". ,", "next, previous frame"),
];

#[component]
pub(crate) fn PaintingView(review: Review) -> NodeId {
    let chosen = create_memo(move || review.painting.with(Option::is_some));
    let nothing = create_memo(move || !chosen.get());
    view! {
        <List spacing=Space::None>
            <Show condition={nothing}>
                <Frame @sizing=ItemSize::Percent(100.0) align=Align::Center>
                    <Text style=TextStyle::PARAGRAPH @test_id={"paint_review.nothing"}>
                        "No paintings changed"
                    </Text>
                </Frame>
            </Show>
            <Show condition={chosen}>
                <Header review={review} />
                <Controls review={review} />
                <Separator />
                <Stage @sizing=ItemSize::Percent(100.0) review={review} />
                <Separator />
                <Shortcuts />
            </Show>
        </List>
    }
}

#[component]
fn Header(review: Review) -> NodeId {
    let name = create_memo(move || {
        review.painting.with(|painting| {
            painting
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default()
        })
    });
    let status = create_memo(move || {
        review
            .painting
            .with(|painting| painting.as_ref().map_or(Status::Changed, |p| p.status))
    });
    let detail = create_memo(move || {
        review.painting.with(|painting| {
            painting
                .as_ref()
                .map(|p| p.detail.clone())
                .unwrap_or_default()
        })
    });
    let detailed = create_memo(move || !detail.get().is_empty());
    view! {
        <Frame padding={Padding::all(PADDING).bottom(SPACING)}>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        style=TextStyle::HEADING
                        family=Family::Mono
                        overflow=Overflow::Ellipsis
                        @test_id={"paint_review.name"}
                    >
                        {name}
                    </Text>
                    <Dynamic value={status}>|status: Status|
                        <StatusBadge status />
                    </Dynamic>
                </List>
                <Show condition={detailed}>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.detail"}>{detail}</Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn StatusBadge(status: Status) -> NodeId {
    view! {
        <Frame
            border={Some(Border::new(role(status)))}
            radius=Radius::Full
            padding=BADGE_PADDING
            @test_id={"paint_review.status"}
        >
            <Text size=TextSize::Sm color={color(status)} align=TextAlign::Center>
                <Icon glyph={glyph(status).to_owned()} />
                {label(status).to_owned()}
            </Text>
        </Frame>
    }
}

#[component]
fn Controls(review: Review) -> NodeId {
    let count = create_memo(move || review.frame_count());
    let many = create_memo(move || count.get() > 1);
    let first = create_memo(move || review.frame_index.get() == 0);
    let last = create_memo(move || review.frame_index.get() + 1 >= count.get());
    let position = create_memo(move || {
        let number = review
            .frame
            .with(|frame| frame.as_ref().map_or(0, |frame| frame.number));
        format!(
            "Frame {number}, {} of {} that changed",
            review.frame_index.get() + 1,
            count.get()
        )
    });
    let pixels = create_memo(move || {
        review.frame.with(
            |frame| match frame.as_ref().and_then(|frame| frame.changed_pixels) {
                Some(1) => "1 pixel differs".to_owned(),
                Some(count) => format!("{count} pixels differ"),
                None => String::new(),
            },
        )
    });
    let counted = create_memo(move || !pixels.get().is_empty());
    view! {
        <Frame padding={Padding::horizontal(PADDING).bottom(SPACING)}>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING wrap=true>
                <ModeButton review mode=Mode::Before />
                <ModeButton review mode=Mode::After />
                <ModeButton review mode=Mode::Changes />
                <ModeButton review mode=Mode::SideBySide />
                <Show condition={counted}>
                    <Text style=TextStyle::CAPTION @test_id={"paint_review.pixels"}>{pixels}</Text>
                </Show>
                <Show condition={many}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_LEFT
                            label="The frame before"
                            disabled={first}
                            @test_id={"paint_review.frame.previous"}
                            on_click={move || review.step_frame(-1)}
                        />
                        <Text style=TextStyle::CAPTION @test_id={"paint_review.frame"}>
                            {position}
                        </Text>
                        <Button
                            icon_only=true
                            glyph=ICON_CHEVRON_RIGHT
                            label="The frame after"
                            disabled={last}
                            @test_id={"paint_review.frame.next"}
                            on_click={move || review.step_frame(1)}
                        />
                    </List>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ModeButton(review: Review, mode: Mode) -> NodeId {
    let pressed = create_memo(move || review.mode.get() == Some(mode));
    let unavailable = create_memo(move || {
        !review
            .frame
            .with(|frame| frame.as_ref().is_some_and(|frame| mode.available(frame)))
    });
    view! {
        <ToggleButton
            label={mode.label().to_owned()}
            pressed={Binding::new(pressed, move |_: bool| review.choose(mode))}
            disabled={unavailable}
            @test_id={format!("paint_review.mode.{}", mode.id())}
        />
    }
}

#[component]
fn Shortcuts() -> NodeId {
    let [flip, modes, paintings, frames] = SHORTCUTS;
    view! {
        <Frame padding={Padding::symmetric(PADDING, SPACING)}>
            <List
                direction=Direction::Horizontal
                align=Align::Center
                spacing=PADDING
                wrap=true
                @test_id={"paint_review.shortcuts"}
            >
                <ShortcutHint hint=flip />
                <ShortcutHint hint=modes />
                <ShortcutHint hint=paintings />
                <ShortcutHint hint=frames />
            </List>
        </Frame>
    }
}

#[component]
fn ShortcutHint(hint: (&'static str, &'static str)) -> NodeId {
    let (keys, description) = hint;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=HINT_SPACING>
            <Chip label={keys.to_owned()} />
            <Text style=TextStyle::CAPTION>{description.to_owned()}</Text>
        </List>
    }
}
