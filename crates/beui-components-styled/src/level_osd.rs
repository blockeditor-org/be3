use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::text::IconSized;
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::color::Color32;
use beui_core::geometry::{Rect, pos2};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, Direction, ForEach, Frame, List, Memo, Prop, ReadSignal, clone, component_accessibility,
    create_effect, create_memo, create_signal, create_timer, now, untrack, use_screens,
};

pub const OSD_DURATION: Duration = Duration::from_millis(1500);

const FADE: Duration = Duration::from_millis(250);
const FRAME: Duration = Duration::from_millis(16);
const SCREEN_HEIGHT_FRACTION: f32 = 0.85;
const MARGIN: f32 = 48.0;
const BOTTOM_MARGIN: f32 = 96.0;
const PADDING_HORIZONTAL: f32 = 20.0;
const PADDING_VERTICAL: f32 = 16.0;
const SPACING: f32 = 16.0;
const GLYPH_SIZE: f32 = 26.0;
const BAR_WIDTH: f32 = 200.0;
const BAR_HEIGHT: f32 = 6.0;
const BAR_RADIUS: u8 = 3;

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
    let since: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
    let (opacity, set_opacity) = create_signal(0.0_f32);
    let ticking = create_timer(clone!(since set_opacity -> move || {
        let started = since.get()?;
        let elapsed = now().saturating_duration_since(started);
        if elapsed < duration {
            return Some(duration - elapsed);
        }
        let faded = (elapsed - duration).as_secs_f32() / FADE.as_secs_f32();
        if faded >= 1.0 {
            since.set(None);
            set_opacity.set(0.0);
            return None;
        }
        set_opacity.set(1.0 - faded);
        Some(FRAME)
    }));
    create_effect(move || {
        let shown = shown.get();
        untrack(|| {
            if shown == 0 {
                return;
            }
            since.set(Some(now()));
            set_opacity.set(1.0);
            ticking.restart(duration);
        });
    });
    let level = create_memo(move || level.get());
    let open = create_memo(clone!(opacity level -> move || {
        opacity.get() > 0.0 && level.with(Option::is_some)
    }));
    let screens = use_screens();
    let places = create_memo(clone!(screens -> move || {
        (0..screens.with(Vec::len)).collect::<Vec<usize>>()
    }));
    view! {
        <List spacing=0.0>
            <ForEach keys={places}>
                {move |index: usize| {
                    let anchored = create_memo(clone!(screens -> move || {
                        let screen = screens.with(|screens| {
                            screens.get(index).map_or(Rect::ZERO, |screen| screen.rect)
                        });
                        OverlayAnchor::Point(pos2(
                            screen.center().x,
                            screen.top() + screen.height() * SCREEN_HEIGHT_FRACTION,
                        ))
                    }));
                    let (open, level, opacity) = (open.clone(), level.clone(), opacity.clone());
                    let id = format!("{id}.{index}");
                    view! {
                        <Overlay
                            anchor={anchored}
                            placement=Placement::Around
                            mode=OverlayMode::Passive
                            traps_focus=false
                            open={open}
                        >
                            <OsdCard level opacity id />
                        </Overlay>
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn OsdCard(level: Memo<Option<OsdLevel>>, opacity: ReadSignal<f32>, id: String) -> NodeId {
    let theme = use_theme();
    let faded = move |color: Memo<Color32>| {
        let opacity = opacity.clone();
        create_memo(move || color.get().scale_alpha(opacity.get()))
    };
    let shown = level.clone();
    let muted =
        create_memo(move || shown.with(|level| level.as_ref().is_some_and(|level| level.muted)));
    let raised = theme.surface_raised.clone();
    let border = theme.border.clone();
    let track = theme.track.clone();
    let (text, text_muted, accent) = (
        theme.text.clone(),
        theme.text_muted.clone(),
        theme.accent.clone(),
    );
    let card = faded(create_memo(move || raised.get()));
    let outline = faded(create_memo(move || border.get()));
    let rail = faded(create_memo(move || track.get()));
    let glyph_color = faded(create_memo(
        clone!(muted text_muted -> move || match muted.get() {
            true => text_muted.get(),
            false => text.get(),
        }),
    ));
    let fill = faded(create_memo(move || match muted.get() {
        true => text_muted.get(),
        false => accent.get(),
    }));
    let glyph = create_memo(clone!(level -> move || {
        level.with(|level| level.as_ref().map(|level| level.glyph.clone()).unwrap_or_default())
    }));
    let filled = create_memo(clone!(level -> move || {
        level.with(|level| level.as_ref().map(|level| level.level.clamp(0.0, 1.0)))
    }));
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
        <Frame padding_horizontal=MARGIN padding_vertical=MARGIN padding_bottom=Some(BOTTOM_MARGIN)>
            <Frame
                color={card}
                outline={outline}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=CARD_RADIUS
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
                @test_id={id}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <IconSized glyph={glyph} font_size=GLYPH_SIZE color={glyph_color} />
                    <Frame
                        width=Some(BAR_WIDTH)
                        height=BAR_HEIGHT
                        color={rail}
                        radius=BAR_RADIUS
                        align_horizontal=Align::Start
                    >
                        <Frame width_fraction={filled} color={fill} radius=BAR_RADIUS />
                    </Frame>
                </List>
            </Frame>
        </Frame>
    }
}
