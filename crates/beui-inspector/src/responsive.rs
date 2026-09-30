use std::rc::Rc;

use beui_components_styled::{
    ButtonVariant, Caption, IconButton, NumberInput, Select, Separator, Theme, select_open,
};
use beui_components_unstyled::ChoiceOption;
use beui_core::document::Document;
use beui_core::geometry::{Vec2, vec2};
use beui_core::icons::{ICON_CLOSE, ICON_SCREEN_ROTATION};
use beui_core::node::NodeId;
use beui_core::screen_simulation::{MAXIMUM_SIZE, MINIMUM_SIZE, ScreenSimulation};
use beui_view::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, NodeRef, ReadSignal, Spacer, WriteSignal,
    clone, component, create_memo, create_signal, view,
};

use super::State;

pub const HEIGHT: f32 = 51.0;
const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 8.0;
const SPACING: f32 = 6.0;
const DEVICE_WIDTH: f32 = 140.0;
const DIMENSION_WIDTH: f32 = 72.0;
const ZOOM_WIDTH: f32 = 84.0;
const THEME: Theme = Theme::DARK;

pub const DEVICES: [(&str, Vec2); 9] = [
    ("Responsive", Vec2::ZERO),
    ("iPhone SE", Vec2::new(375.0, 667.0)),
    ("iPhone 14", Vec2::new(390.0, 844.0)),
    ("Pixel 8", Vec2::new(412.0, 915.0)),
    ("Galaxy S24", Vec2::new(360.0, 780.0)),
    ("iPad Mini", Vec2::new(744.0, 1133.0)),
    ("iPad Pro", Vec2::new(1024.0, 1366.0)),
    ("Laptop", Vec2::new(1280.0, 800.0)),
    ("Desktop", Vec2::new(1920.0, 1080.0)),
];
pub const ZOOMS: [(&str, Option<f32>); 6] = [
    ("Fit", None),
    ("50%", Some(0.5)),
    ("75%", Some(0.75)),
    ("100%", Some(1.0)),
    ("150%", Some(1.5)),
    ("200%", Some(2.0)),
];
pub const DEFAULT: ScreenSimulation = ScreenSimulation {
    size: DEVICES[2].1,
    zoom: None,
};

pub struct Toolbar {
    pub document: Document,
    pub set_screen: WriteSignal<ScreenSimulation>,
    device: NodeRef,
    zoom: NodeRef,
}

impl Toolbar {
    pub fn open(&self) -> bool {
        [&self.device, &self.zoom].into_iter().any(|select| {
            select
                .try_get()
                .is_some_and(|select| select_open(&self.document, select))
        })
    }
}

pub fn build(state: &Rc<State>) -> Toolbar {
    let (screen, set_screen) = create_signal(state.screen_simulation.get().unwrap_or(DEFAULT));
    let (device, zoom) = (NodeRef::new(), NodeRef::new());
    let document = beui_view::reactive::build(clone!(state device zoom -> move || {
        view! {
            <ResponsiveToolbar state screen device zoom />
        }
    }));
    Toolbar {
        document,
        set_screen,
        device,
        zoom,
    }
}

fn device_index(size: Vec2) -> usize {
    DEVICES
        .iter()
        .skip(1)
        .position(|(_, device)| *device == size || *device == vec2(size.y, size.x))
        .map_or(0, |index| index + 1)
}

fn labels<T>(options: &[(&'static str, T)]) -> Vec<&'static str> {
    options.iter().map(|(label, _)| *label).collect()
}

#[component]
fn ResponsiveToolbar(
    state: Rc<State>,
    screen: ReadSignal<ScreenSimulation>,
    device: NodeRef,
    zoom: NodeRef,
) -> NodeId {
    let selected_device = create_memo(clone!(screen -> move || {
        Some(device_index(screen.get().size))
    }));
    let width = create_memo(clone!(screen -> move || f64::from(screen.get().size.x)));
    let height = create_memo(clone!(screen -> move || f64::from(screen.get().size.y)));
    let selected_zoom = create_memo(clone!(screen -> move || {
        let zoom = screen.get().zoom;
        ZOOMS.iter().position(|(_, candidate)| *candidate == zoom)
    }));
    let (device_state, width_state, height_state) = (state.clone(), state.clone(), state.clone());
    let (rotate_state, zoom_state, close_state) = (state.clone(), state.clone(), state);
    view! {
        <List spacing=0.0>
            <Frame
                color={THEME.surface}
                radius=0
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Frame width=DEVICE_WIDTH>
                        <Select
                            @node_ref=&device
                            @test_id={"inspector.responsive.device"}
                            options={view! {
                                <ForEach keys={labels(&DEVICES)}>
                                    {|label: &'static str| view! {
                                        <ChoiceOption label />
                                    }}
                                </ForEach>
                            }}
                            selected={selected_device}
                            label="Device"
                            on_change={move |index: Option<usize>| {
                                if let Some((_, size)) = index.and_then(|index| DEVICES.get(index))
                                    && *size != Vec2::ZERO
                                {
                                    device_state.update_screen(|screen| screen.resized(*size));
                                }
                            }}
                        />
                    </Frame>
                    <Frame width=DIMENSION_WIDTH>
                        <NumberInput
                            @test_id={"inspector.responsive.width"}
                            value={width}
                            min={f64::from(MINIMUM_SIZE)}
                            max={f64::from(MAXIMUM_SIZE)}
                            label="Width"
                            on_change={move |width: f64| {
                                width_state.update_screen(|screen| {
                                    screen.resized(vec2(width as f32, screen.size.y))
                                });
                            }}
                        />
                    </Frame>
                    <Caption content="x" />
                    <Frame width=DIMENSION_WIDTH>
                        <NumberInput
                            @test_id={"inspector.responsive.height"}
                            value={height}
                            min={f64::from(MINIMUM_SIZE)}
                            max={f64::from(MAXIMUM_SIZE)}
                            label="Height"
                            on_change={move |height: f64| {
                                height_state.update_screen(|screen| {
                                    screen.resized(vec2(screen.size.x, height as f32))
                                });
                            }}
                        />
                    </Frame>
                    <IconButton
                        @test_id={"inspector.responsive.rotate"}
                        glyph={ICON_SCREEN_ROTATION.to_owned()}
                        label="Rotate"
                        variant=ButtonVariant::Ghost
                        on_click={move || rotate_state.update_screen(ScreenSimulation::rotated)}
                    />
                    <Frame width=ZOOM_WIDTH>
                        <Select
                            @node_ref=&zoom
                            @test_id={"inspector.responsive.zoom"}
                            options={view! {
                                <ForEach keys={labels(&ZOOMS)}>
                                    {|label: &'static str| view! {
                                        <ChoiceOption label />
                                    }}
                                </ForEach>
                            }}
                            selected={selected_zoom}
                            label="Zoom"
                            on_change={move |index: Option<usize>| {
                                if let Some((_, zoom)) = index.and_then(|index| ZOOMS.get(index)) {
                                    zoom_state.update_screen(|screen| ScreenSimulation {
                                        zoom: *zoom,
                                        ..screen
                                    });
                                }
                            }}
                        />
                    </Frame>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <IconButton
                        @test_id={"inspector.responsive.close"}
                        glyph={ICON_CLOSE.to_owned()}
                        label="Close responsive design mode"
                        variant=ButtonVariant::Ghost
                        on_click={move || close_state.simulate_screen(None)}
                    />
                </List>
            </Frame>
            <Separator />
        </List>
    }
}
