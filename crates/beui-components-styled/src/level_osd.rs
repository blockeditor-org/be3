use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::layer::{LayerLevel, LayerPolicy};
use std::time::{Duration, Instant};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::use_theme;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::geometry::{Rect, pos2};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub const OSD_DURATION: Duration = Duration::from_millis(1500);

const FADE: Duration = Duration::from_millis(250);
const SCREEN_HEIGHT_FRACTION: f32 = 0.85;
const MARGIN: f32 = 48.0;
const BOTTOM_MARGIN: f32 = 96.0;
const PADDING_HORIZONTAL: f32 = 20.0;
const PADDING_VERTICAL: f32 = 16.0;
const SPACING: f32 = 16.0;
const BAR_WIDTH: f32 = 200.0;
const BAR_HEIGHT: f32 = 6.0;
const BAR_RADIUS: f32 = 3.0;

#[derive(Clone, Debug, PartialEq)]
pub struct OsdLevel {
    pub glyph: String,
    pub label: String,
    pub level: f32,
    pub muted: bool,
}

#[component]
pub fn LevelOsd(
    level: Prop<Option<OsdLevel>>,
    shown: Prop<u64>,
    #[prop(default = OSD_DURATION)] duration: Duration,
    #[prop(default = "level-osd".to_owned())] id: String,
) -> NodeId {
    let since = StoredCell::new(None::<Instant>);
    let opacity = create_signal(0.0_f32);
    let step = move |at: Instant| {
        let Some(started) = since.get() else {
            return false;
        };
        let progress = tween(
            at.saturating_duration_since(started)
                .saturating_sub(duration)
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
            ticking.restart(duration);
        });
    });
    let level = create_memo(move || level.get());
    let open = create_memo(move || opacity.get() > 0.0 && level.with(Option::is_some));
    let screens = use_screens();
    let places = create_memo(move || (0..screens.with(Vec::len)).collect::<Vec<usize>>());
    view! {
        <List spacing=0.0>
            <ForEach keys={places}>|index: usize| {
                let anchored = create_memo(move || {
                    let screen = screens.with(|screens| {
                        screens.get(index).map_or(Rect::ZERO, |screen| screen.rect)
                    });
                    OverlayAnchor::Point(pos2(
                        screen.center().x,
                        screen.top() + screen.height() * SCREEN_HEIGHT_FRACTION,
                    ))
                });
                let id = format!("{id}.{index}");
                view! {
                    <Overlay
                        anchor={anchored}
                        placement=Placement::Around
                        policy=LayerPolicy::Inert
                        level=LayerLevel::Top
                        traps_focus=false
                        open={open}
                        content={move || view! {
                            <OsdCard level opacity={opacity.read_only()} id />
                        }}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn OsdCard(level: Memo<Option<OsdLevel>>, opacity: ReadSignal<f32>, id: String) -> NodeId {
    let theme = use_theme();
    let faded =
        move |color: Memo<Color32>| create_memo(move || color.get().scale_alpha(opacity.get()));
    let shown = level;
    let muted =
        create_memo(move || shown.with(|level| level.as_ref().is_some_and(|level| level.muted)));
    let raised = theme.surface_raised;
    let border = theme.border;
    let track = theme.track;
    let (text, text_muted, accent) = (theme.text, theme.text_muted, theme.accent);
    let card = faded(create_memo(move || raised.get()));
    let outline = faded(create_memo(move || border.get()));
    let rail = faded(create_memo(move || track.get()));
    let glyph_color = faded(create_memo(move || match muted.get() {
        true => text_muted.get(),
        false => text.get(),
    }));
    let glyph_ink = create_memo(move || TextColor::Custom(glyph_color.get()));
    let fill = faded(create_memo(move || match muted.get() {
        true => text_muted.get(),
        false => accent.get(),
    }));
    let glyph = create_memo(move || {
        level.with(|level| {
            level
                .as_ref()
                .map(|level| level.glyph.clone())
                .unwrap_or_default()
        })
    });
    let filled = create_memo(move || {
        level.with(|level| Extent {
            fraction: level.as_ref().map(|level| level.level.clamp(0.0, 1.0)),
            ..Extent::INTRINSIC
        })
    });
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::ProgressIndicator);
        if let Some(level) = level.get() {
            node.set_label(level.label);
            node.set_numeric_value(f64::from(level.level.clamp(0.0, 1.0)));
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(1.0);
        }
        node
    }));
    view! {
        <Frame padding={Sides::all(MARGIN).bottom(BOTTOM_MARGIN)}>
            <Frame
                fill={ColorRole::custom(card)}
                border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get()))))}
                radius=Radius::Lg
                padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id={id}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text size=TextSize::Xl color={glyph_ink} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                    <Frame
                        width=BAR_WIDTH
                        height=BAR_HEIGHT
                        fill={ColorRole::custom(rail)}
                        radius={Radius::custom(BAR_RADIUS)}
                        align={Align2::horizontal(Align::Start)}
                    >
                        <Frame
                            width=filled
                            fill={ColorRole::custom(fill)}
                            radius={Radius::custom(BAR_RADIUS)}
                        />
                    </Frame>
                </List>
            </Frame>
        </Frame>
    }
}
