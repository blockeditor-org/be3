use beui_components_styled::*;
use beui_components_unstyled::ChoiceOption;
use beui_core::document::Document;
use beui_core::geometry::{Vec2, vec2};
use beui_core::icons::{ICON_ROTATE_CW_SQUARE, ICON_X};
use beui_core::node::NodeId;
use beui_core::screen_simulation::{MAXIMUM_SIZE, MINIMUM_SIZE, ScreenSimulation};
use beui_view::reactive::*;

use super::State;

pub const HEIGHT: f32 = 51.0;
const PADDING: Space = Space::Md;
const SPACING: Space = Space::Sm;
const DEVICE_WIDTH: Size = Size::Custom(140.0);
const DIMENSION_WIDTH: Size = Size::Custom(72.0);
const ZOOM_WIDTH: Size = Size::Custom(84.0);

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
    pub screen: Signal<ScreenSimulation>,
    device: NodeRef,
    zoom: NodeRef,
    _scope: Scope,
}

impl Toolbar {
    pub fn open(&self) -> bool {
        let device = self
            .device
            .try_get()
            .is_some_and(|select| self.document.api::<SelectApi<&'static str>>(select).open());
        let zoom = self
            .zoom
            .try_get()
            .is_some_and(|select| self.document.api::<SelectApi<Option<f32>>>(select).open());
        device || zoom
    }
}

pub fn build(state: Stored<State>) -> Toolbar {
    let owner = RootScope::new();
    let screen = create_signal(state.value().screen_simulation.get().unwrap_or(DEFAULT));
    let (device, zoom) = (NodeRef::new(), NodeRef::new());
    let mut document = Document::new();
    let root = enter(&mut document, move || {
        provide_context(ThemeStore::new(Theme::DARK));
        view! {
            <ResponsiveToolbar state screen={screen.read_only()} device zoom />
        }
    });
    document.set_root(root);
    Toolbar {
        document,
        screen,
        device,
        zoom,
        _scope: owner.into_scope(),
    }
}

fn device_index(size: Vec2) -> usize {
    DEVICES
        .iter()
        .skip(1)
        .position(|(_, device)| *device == size || *device == vec2(size.y, size.x))
        .map_or(0, |index| index + 1)
}

fn zoom_of(label: &str) -> Option<f32> {
    ZOOMS
        .iter()
        .find(|(candidate, _)| *candidate == label)
        .map(|(_, zoom)| *zoom)
        .expect("a label from ZOOMS")
}

fn labels<T>(options: &[(&'static str, T)]) -> Vec<&'static str> {
    options.iter().map(|(label, _)| *label).collect()
}

#[component]
fn ResponsiveToolbar(
    state: Stored<State>,
    screen: ReadSignal<ScreenSimulation>,
    device: NodeRef,
    zoom: NodeRef,
) -> NodeId {
    let selected_device = create_memo(move || Some(DEVICES[device_index(screen.get().size)].0));
    let width = create_memo(move || f64::from(screen.get().size.x));
    let height = create_memo(move || f64::from(screen.get().size.y));
    let selected_zoom = create_memo(move || Some(screen.get().zoom));
    view! {
        <List spacing=Space::None>
            <Frame fill=ColorRole::Surface padding=PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Frame width=DEVICE_WIDTH>
                        <Select
                            @node_ref=&device
                            @test_id={"inspector.responsive.device"}
                            options={view! {
                                <ForEach keys={labels(&DEVICES)}>|label: &'static str|
                                    <ChoiceOption value=label label />
                                </ForEach>
                            }}
                            selected={Binding::new(selected_device, move |label: Option<&'static str>| {
                                if let Some((_, size)) = DEVICES.iter().find(|(candidate, _)| Some(*candidate) == label)
                                    && *size != Vec2::ZERO
                                {
                                    state.value().update_screen(|screen| screen.resized(*size));
                                }
                            })}
                            label="Device"
                        />
                    </Frame>
                    <Frame width=DIMENSION_WIDTH>
                        <NumberInput
                            @test_id={"inspector.responsive.width"}
                            value={Binding::new(width, move |width: f64| {
                                state.value().update_screen(|screen| {
                                    screen.resized(vec2(width as f32, screen.size.y))
                                });
                            })}
                            min={f64::from(MINIMUM_SIZE)}
                            max={f64::from(MAXIMUM_SIZE)}
                            label="Width"
                        />
                    </Frame>
                    <Text style=TextStyle::CAPTION>"x"</Text>
                    <Frame width=DIMENSION_WIDTH>
                        <NumberInput
                            @test_id={"inspector.responsive.height"}
                            value={Binding::new(height, move |height: f64| {
                                state.value().update_screen(|screen| {
                                    screen.resized(vec2(screen.size.x, height as f32))
                                });
                            })}
                            min={f64::from(MINIMUM_SIZE)}
                            max={f64::from(MAXIMUM_SIZE)}
                            label="Height"
                        />
                    </Frame>
                    <Button
                        icon_only=true
                        @test_id={"inspector.responsive.rotate"}
                        glyph=ICON_ROTATE_CW_SQUARE
                        label="Rotate"
                        variant=ButtonVariant::Ghost
                        on_click={move || state.value().update_screen(ScreenSimulation::rotated)}
                    />
                    <Frame width=ZOOM_WIDTH>
                        <Select
                            @node_ref=&zoom
                            @test_id={"inspector.responsive.zoom"}
                            options={view! {
                                <ForEach keys={labels(&ZOOMS)}>|label: &'static str|
                                    <ChoiceOption value={zoom_of(label)} label />
                                </ForEach>
                            }}
                            selected={Binding::new(selected_zoom, move |zoom: Option<Option<f32>>| {
                                if let Some(zoom) = zoom {
                                    state.value().update_screen(|screen| ScreenSimulation {
                                        zoom,
                                        ..screen
                                    });
                                }
                            })}
                            label="Zoom"
                        />
                    </Frame>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button
                        icon_only=true
                        @test_id={"inspector.responsive.close"}
                        glyph=ICON_X
                        label="Close responsive design mode"
                        variant=ButtonVariant::Ghost
                        on_click={move || state.value().simulate_screen(None)}
                    />
                </List>
            </Frame>
            <Separator />
        </List>
    }
}
