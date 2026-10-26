use be_block::display_settings::Rotation;
use beui::accesskit::{Node as AccessNode, Role, Toggled};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled;
use beui::unstyled::ButtonHandle;
use beui::{NodeId, TextAlign};

use crate::HostDisplay;

const ITEM_WIDTH: f32 = 64.0;
const TILE: f32 = 56.0;
const SCREEN: f32 = 42.0;
const TITLE_BAR: f32 = 7.0;
const ITEM_SPACING: f32 = 4.0;

fn quarters(rotation: Rotation) -> u8 {
    (rotation.degrees() / 90) as u8
}

fn drawn_turns(tile: Rotation, current: Rotation) -> u8 {
    (4 + quarters(tile) - quarters(current)) % 4
}

fn rotation_name(rotation: Rotation, landscape_panel: bool) -> &'static str {
    let wide = match rotation {
        Rotation::Normal | Rotation::Clockwise180 => landscape_panel,
        Rotation::Clockwise90 | Rotation::Clockwise270 => !landscape_panel,
    };
    match (wide, rotation) {
        (true, Rotation::Normal | Rotation::Clockwise90) => "Landscape",
        (true, _) => "Landscape, flipped",
        (false, Rotation::Normal | Rotation::Clockwise90) => "Portrait",
        (false, _) => "Portrait, flipped",
    }
}

fn screen_size(display: &HostDisplay) -> (f32, f32) {
    let (width, height) = (
        display.current.width.max(1) as f32,
        display.current.height.max(1) as f32,
    );
    let (width, height) = match display.rotation {
        Rotation::Clockwise90 | Rotation::Clockwise270 => (height, width),
        Rotation::Normal | Rotation::Clockwise180 => (width, height),
    };
    let scale = SCREEN / width.max(height);
    (width * scale, height * scale)
}

fn tile_test_id(prefix: &str, rotation: Rotation) -> String {
    format!("{prefix}.{}", rotation.degrees())
}

#[component]
pub fn RotationPicker(
    test_id: String,
    display: Memo<Option<HostDisplay>>,
    chosen: Memo<Rotation>,
    read_only: Memo<bool>,
    on_pick: Callback<Rotation>,
) -> NodeId {
    let current = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .map_or(Rotation::Normal, |display| display.rotation)
        })
    });
    let landscape = create_memo(move || {
        display.with(|display| {
            display
                .as_ref()
                .is_none_or(|display| display.current.width >= display.current.height)
        })
    });
    let size = create_memo(move || {
        display.with(|display| display.as_ref().map_or((SCREEN, SCREEN), screen_size))
    });
    view! {
        <List direction=Direction::Horizontal wrap=true spacing=ITEM_SPACING align=Align::Start>
            <ForEach keys={Rotation::ALL.to_vec()}>|rotation: Rotation|
                <RotationTile
                    rotation={rotation}
                    name={tile_test_id(&test_id, rotation)}
                    current={current}
                    chosen={chosen}
                    landscape={landscape}
                    size={size}
                    read_only={read_only}
                    on_pick={on_pick}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn RotationTile(
    rotation: Rotation,
    name: String,
    current: Memo<Rotation>,
    chosen: Memo<Rotation>,
    landscape: Memo<bool>,
    size: Memo<(f32, f32)>,
    read_only: Memo<bool>,
    on_pick: Callback<Rotation>,
) -> NodeId {
    let turns = create_memo(move || drawn_turns(rotation, current.get()));
    let now = create_memo(move || current.get() == rotation);
    let picked = create_memo(move || chosen.get() == rotation);
    let label = create_memo(move || rotation_name(rotation, landscape.get()).to_owned());
    let accessibility = create_memo(move || {
        let mut node = AccessNode::new(Role::RadioButton);
        node.set_label(label.get());
        node.set_toggled(Toggled::from(picked.get()));
        if now.get() {
            node.set_description("Now");
        }
        node
    });
    view! {
        <unstyled::Button
            @test_id={name}
            accessibility={accessibility}
            disabled={read_only}
            on_click={move || on_pick.call(rotation)}
        >|handle: ButtonHandle| {
            let fill = create_memo(move || match (picked.get(), handle.hovered.get()) {
                (true, _) => ColorRole::AccentSoft,
                (false, true) => ColorRole::Hover,
                (false, false) => ColorRole::None,
            });
            let border = create_memo(move || match picked.get() || handle.focused.get() {
                true => Some(Border::new(ColorRole::Accent).width(2.0)),
                false => Some(Border::DEFAULT),
            });
            view! {
                <Frame width=ITEM_WIDTH>
                    <List spacing=ITEM_SPACING align=Align::Center>
                        <Frame width=TILE height=TILE radius=Radius::Md fill={fill} border={border}>
                            <Turn quarter_turns={turns}>
                                <Frame width=TILE height=TILE align=Align2::CENTER>
                                    <LittleScreen size={size} />
                                </Frame>
                            </Turn>
                        </Frame>
                        <Text style=TextStyle::CAPTION align=TextAlign::Center>{label}</Text>
                        <Show condition={now}>
                            <Text size=TextSize::Xs color=TextColor::Accent>"Now"</Text>
                        </Show>
                    </List>
                </Frame>
            }
        }</unstyled::Button>
    }
}

#[component]
fn LittleScreen(size: Memo<(f32, f32)>) -> NodeId {
    let width = create_memo(move || Extent::px(size.get().0));
    let height = create_memo(move || Extent::px(size.get().1));
    view! {
        <Frame
            width={width}
            height={height}
            fill=ColorRole::Background
            border={Border::DEFAULT}
            radius=Radius::Xs
        >
            <List spacing=0.0>
                <Frame height=TITLE_BAR fill=ColorRole::Accent />
                <Frame @sizing=ItemSize::Percent(100.0) align=Align2::CENTER>
                    <Text size=TextSize::Xs>"Aa"</Text>
                </Frame>
            </List>
        </Frame>
    }
}
