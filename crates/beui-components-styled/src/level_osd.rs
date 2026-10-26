use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::icons::Glyph;
use beui_core::layer::{LayerLevel, LayerPolicy};
use std::time::{Duration, Instant};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::use_theme;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::geometry::{Pos2, Rect};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub const OSD_DURATION: Duration = Duration::from_millis(1500);

const FADE: Duration = Duration::from_millis(250);
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
    pub glyph: Glyph,
    pub label: String,
    pub level: f32,
    pub muted: bool,
}

#[component]
pub fn LevelOsd(
    level: Prop<Option<OsdLevel>>,
    shown: Prop<u64>,
    #[prop(default = OSD_DURATION)] duration: Duration,
    #[prop(default = None)] area: Prop<Option<Rect>>,
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
    let inset = create_memo(move || {
        area.get().map_or(Sides::ZERO, |area| {
            Sides::new(area.left(), area.top(), 0.0, 0.0)
        })
    });
    let outer = create_memo(move || match area.get() {
        Some(_) => Align2::START,
        None => Align2::new(Align::Center, Align::End),
    });
    let width = create_memo(move || area.get().map(|area| area.width()));
    let height = create_memo(move || area.get().map(|area| area.height()));
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            placement=Placement::Fill
            policy=LayerPolicy::Inert
            level=LayerLevel::Top
            traps_focus=false
            open={open}
            content={move || { let id = id.clone(); view! {
                <Frame padding=inset align={outer}>
                    <Frame width=width height=height align={Align2::new(Align::Center, Align::End)}>
                        <OsdCard level opacity={opacity.read_only()} id />
                    </Frame>
                </Frame>
            } }}
        />
    }
}

#[component]
fn OsdCard(level: Memo<Option<OsdLevel>>, opacity: ReadSignal<f32>, id: String) -> NodeId {
    let theme = use_theme();
    let faded =
        move |color: Memo<Color32>| create_memo(move || color.get().scale_alpha(opacity.get()));
    let muted =
        create_memo(move || level.with(|level| level.as_ref().is_some_and(|level| level.muted)));
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
    let glyph = create_memo(move || level.with(|level| level.as_ref().map(|level| level.glyph)));
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
