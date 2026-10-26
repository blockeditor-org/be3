use std::time::{Duration, Instant};

use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{
    Color32, LayerLevel, LayerPolicy, NodeId, OverlayAnchor, Placement, Pos2, TextAlign, pos2,
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
    let screens = use_screens();
    let anchor = create_memo(move || {
        OverlayAnchor::Point(screens.with(|screens| {
            screens
                .iter()
                .map(|screen| screen.rect.min)
                .reduce(|a, b| pos2(a.x.min(b.x), a.y.min(b.y)))
                .unwrap_or(Pos2::ZERO)
        }))
    });
    view! {
        <Overlay
            anchor={anchor}
            placement=Placement::At
            policy=LayerPolicy::Inert
            level=LayerLevel::Top
            traps_focus=false
            open={open}
            content={move || { let id = id.clone(); view! {
                <EveryScreen align={Align2::new(Align::Center, Align::End)}>|slot: ScreenSlot| {
                    let below = create_memo(move || slot.rect.get().height() * (1.0 - SCREEN_HEIGHT_FRACTION));
                    let id = format!("{id}.{}", slot.index);
                    view! {
                        <OsdCard level opacity={opacity.read_only()} below id />
                    }
                }</EveryScreen>
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
    let theme = use_theme();
    let faded = move |color: Memo<Color32>| {
        ColorRole::custom(create_memo(move || color.get().scale_alpha(opacity.get())))
    };
    let muted =
        create_memo(move || level.with(|level| level.as_ref().is_some_and(|level| level.muted)));
    let (raised, border, track) = (theme.surface_raised, theme.border, theme.track);
    let (text, text_muted, accent) = (theme.text, theme.text_muted, theme.accent);
    let card = faded(create_memo(move || raised.get()));
    let outline = create_memo(move || border.get().scale_alpha(opacity.get()));
    let rail = faded(create_memo(move || track.get()));
    let glyph_color = create_memo(move || {
        match muted.get() {
            true => text_muted.get(),
            false => text.get(),
        }
        .scale_alpha(opacity.get())
    });
    let glyph_ink = create_memo(move || TextColor::Custom(glyph_color.get()));
    let fill = faded(create_memo(move || match muted.get() {
        true => text_muted.get(),
        false => accent.get(),
    }));
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
                fill=card
                border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get()))))}
                radius=Radius::Lg
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id={id}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text size=TextSize::Xl color={glyph_ink} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                    <Progress value label width=BAR_WIDTH fill track=rail />
                </List>
            </Frame>
        </Frame>
    }
}
