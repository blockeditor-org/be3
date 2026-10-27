use std::time::{Duration, Instant};

use beui::{Align, Align2, Direction, Sides};
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    LayerLevel, LayerPolicy, NodeId, OverlayAnchor, Placement, Pos2, Rect, TextAlign,
};

pub(crate) const OSD_DURATION: Duration = Duration::from_millis(1500);

const FADE: Duration = Duration::from_millis(250);
const SCREEN_HEIGHT_FRACTION: f32 = 0.85;
const MARGIN: f32 = 48.0;
const PADDING_HORIZONTAL: Space = Space::Xl2;
const PADDING_VERTICAL: Space = Space::Xl;
const SPACING: Space = Space::Xl;
const BAR_WIDTH: Size = Size::Menu;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OsdLevel {
    pub glyph: Glyph,
    pub label: String,
    pub level: f32,
    pub muted: bool,
}

#[component]
pub(crate) fn LevelOsd(
    level: Memo<Option<OsdLevel>>,
    shown: ReadSignal<u64>,
    area: Memo<Rect>,
    id: String,
) -> NodeId {
    let since = StoredCell::new(None::<Instant>);
    let opacity = create_signal(0.0_f32);
    let step = move |at: Instant| {
        let Some(started) = since.get() else {
            return false;
        };
        let progress = tween(
            at.saturating_duration_since(started)
                .saturating_sub(OSD_DURATION)
                .as_secs_f32(),
            FADE.as_secs_f32(),
            Easing::Linear,
        );
        opacity.set(1.0 - progress);
        progress < 1.0
    };
    let fade = create_animation(
        move |frame| step(frame.now),
        move || {
            since.set(None);
            opacity.set(0.0);
        },
    );
    let ticking = create_timer(move || {
        fade.start();
        if fade.running() && !step(now()) {
            fade.finish();
        }
        None
    });
    create_effect(move || {
        let shown = shown.get();
        untrack(|| {
            if shown == 0 {
                return;
            }
            fade.stop();
            since.set(Some(now()));
            opacity.set(1.0);
            ticking.restart(OSD_DURATION);
        });
    });
    let open = create_memo(move || opacity.get() > 0.0 && level.with(Option::is_some));
    let below = create_memo(move || area.get().height() * (1.0 - SCREEN_HEIGHT_FRACTION));
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            placement=Placement::Fill
            policy=LayerPolicy::Inert
            level=LayerLevel::Top
            traps_focus=false
            open={open}
            content={move || { let id = id.clone(); view! {
                <Frame
                    width=Length::FILL
                    height=Length::FILL
                    align={Align2::new(Align::Center, Align::End)}
                >
                    <OsdCard level opacity={opacity.read_only()} below id />
                </Frame>
            } }}
        />
    }
}

#[component]
fn OsdCard(
    level: Memo<Option<OsdLevel>>,
    opacity: ReadSignal<f32>,
    below: Memo<f32>,
    id: String,
) -> NodeId {
    let muted =
        create_memo(move || level.with(|level| level.as_ref().is_some_and(|level| level.muted)));
    let glyph_ink = create_memo(move || match muted.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let fill = create_memo(move || match muted.get() {
        true => ColorRole::TextMuted,
        false => ColorRole::Accent,
    });
    let glyph = create_memo(move || level.with(|level| level.as_ref().map(|level| level.glyph)));
    let value = create_memo(move || level.with(|level| level.as_ref().map(|level| level.level)));
    let label = create_memo(move || {
        level.with(|level| {
            level
                .as_ref()
                .map(|level| level.label.clone())
                .unwrap_or_default()
        })
    });
    view! {
        <Frame
            padding={create_memo(move || Padding::custom(Sides::all(MARGIN).bottom(below.get())))}
        >
            <Frame
                opacity={opacity}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Lg
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id={id}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text size=TextSize::Xl color={glyph_ink} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                    <Progress value label width=BAR_WIDTH fill />
                </List>
            </Frame>
        </Frame>
    }
}
