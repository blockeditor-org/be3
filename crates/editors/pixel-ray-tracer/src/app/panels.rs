use block_editor_beui::be_block::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_PALETTE, RayEntity, RaySettings,
};
use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::accesskit::{Node, Role};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, List, Prop, Show, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, Slider, ToggleButton, Tooltip, use_theme,
};
use block_editor_beui::beui::unstyled::{Pressable, SliderScale};

use crate::overlay::palette_color;

use super::state::{RayState, Surface, Tool};

const SWATCH: f32 = 18.0;

#[component]
pub(crate) fn ToolsPanel(state: RayState) -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing=8.0>
            <Heading content="Tools" />
            <ForEach keys={Tool::ALL.to_vec()}>
                {move |tool: Tool| {
                    view! {
                        <ToolButton state tool />
                    }
                }}
            </ForEach>
            <Caption
                content="Scroll: pan · Ctrl scroll: zoom · Middle drag: pan · Delete: remove entity"
                wrap=true
                color={theme.text_muted}
            />
        </List>
    }
}

#[component]
fn ToolButton(state: RayState, tool: Tool) -> NodeId {
    let chosen = state.tool;
    let pressed = create_memo(move || chosen.get() == tool);
    view! {
        <ToggleButton
            label={tool.label().to_owned()}
            pressed={pressed}
            @test_id={format!("pixel_ray_tracer.tool.{}", tool.label())}
            on_change={move |_| state.choose_tool(tool)}
        />
    }
}

#[component]
pub(crate) fn PropertiesPanel(state: RayState) -> NodeId {
    let selected = create_memo(move || state.selected_entity());
    let chosen = create_memo(move || selected.get().is_some());
    let unchosen = create_memo(move || !chosen.get());
    view! {
        <List spacing=10.0>
            <Heading content="Palette" />
            <Palette state={state} />
            <Show condition={chosen}>
                <SelectedEntity state={state} selected={selected} />
            </Show>
            <Show condition={unchosen}>
                <NewEntity state={state} />
            </Show>
            <RaySettingsPanels state={state} />
        </List>
    }
}

#[component]
fn Palette(state: RayState) -> NodeId {
    let indexes = (0..PIXEL_RAY_TRACER_PALETTE.len()).collect::<Vec<usize>>();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
            <ForEach keys={indexes}>
                {move |index: usize| {
                    view! {
                        <Swatch state index />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn Swatch(state: RayState, index: usize) -> NodeId {
    let color = palette_color(index as u8);
    let chosen = state.color_index;
    let picked = create_memo(move || usize::from(chosen.get()) == index);
    let theme = use_theme();
    let outline = create_memo(move || match picked.get() {
        true => theme.accent.get(),
        false => Color32::TRANSPARENT,
    });
    let label = format!("Color {}", index + 1);
    let described = label.clone();
    view! {
        <Tooltip label={label}>
            <Pressable
                accessibility={swatch_role(&described)}
                @test_id={format!("pixel_ray_tracer.color.{index}")}
                on_click={move || state.choose_color(index as u8)}
            >
                <Frame
                    width=SWATCH
                    height=SWATCH
                    color={color}
                    radius=3
                    outline={outline}
                    outline_width=2.0
                    outline_visible={picked}
                />
            </Pressable>
        </Tooltip>
    }
}

fn swatch_role(label: &str) -> Prop<Node> {
    let mut node = Node::new(Role::Button);
    node.set_label(label.to_owned());
    Prop::Static(node)
}

#[component]
fn SelectedEntity(
    state: RayState,
    selected: block_editor_beui::beui::reactive::Memo<Option<RayEntity>>,
) -> NodeId {
    let kind = create_memo(move || {
        match selected.get() {
            Some(RayEntity::Surface { .. }) => "Surface",
            Some(RayEntity::Light { .. }) => "Light",
            Some(RayEntity::Water { .. }) => "Water",
            None => "",
        }
        .to_owned()
    });
    let surface = create_memo(move || matches!(selected.get(), Some(RayEntity::Surface { .. })));
    let light = create_memo(move || matches!(selected.get(), Some(RayEntity::Light { .. })));
    let field = |read: fn(&RayEntity) -> f32| {
        create_memo(move || selected.get().as_ref().map_or(0.0, read))
    };
    let roughness = field(|entity| match entity {
        RayEntity::Surface { roughness, .. } => *roughness,
        _ => 0.0,
    });
    let metalness = field(|entity| match entity {
        RayEntity::Surface { metalness, .. } => *metalness,
        _ => 0.0,
    });
    let transmission = field(|entity| match entity {
        RayEntity::Surface { transmission, .. } => *transmission,
        _ => 0.0,
    });
    let ior = field(|entity| match entity {
        RayEntity::Surface {
            refractive_index, ..
        } => *refractive_index,
        _ => 1.5,
    });
    let intensity = field(|entity| match entity {
        RayEntity::Light { intensity, .. } => *intensity,
        _ => 1.0,
    });
    let edit = move |selected: &block_editor_beui::beui::reactive::Memo<Option<RayEntity>>,
                     write: fn(&mut RayEntity, f32)| {
        let selected = *selected;
        move |value: f32| {
            let Some(mut entity) = selected.get_untracked() else {
                return;
            };
            write(&mut entity, value);
            state.update_entity(entity);
        }
    };
    let set_intensity = edit(&selected, |entity, value| {
        if let RayEntity::Light { intensity, .. } = entity {
            *intensity = value;
        }
    });
    let set_roughness = edit(&selected, |entity, value| {
        if let RayEntity::Surface { roughness, .. } = entity {
            *roughness = value;
        }
    });
    let set_metalness = edit(&selected, |entity, value| {
        if let RayEntity::Surface { metalness, .. } = entity {
            *metalness = value;
        }
    });
    let set_transmission = edit(&selected, |entity, value| {
        if let RayEntity::Surface { transmission, .. } = entity {
            *transmission = value;
        }
    });
    let set_ior = edit(&selected, |entity, value| {
        if let RayEntity::Surface {
            refractive_index, ..
        } = entity
        {
            *refractive_index = value;
        }
    });
    view! {
        <List spacing=8.0>
            <Heading content="Selected entity" />
            <Body content={kind} />
            <Show condition={surface}>
                {move || view! {
                    <List spacing=6.0>
                        <Slider value={roughness} label="Roughness" on_change={set_roughness} />
                        <Slider value={metalness} label="Metalness" on_change={set_metalness} />
                        <Slider
                            value={transmission}
                            label="Transmission"
                            on_change={set_transmission}
                        />
                        <Slider value={ior} min=1.0 max=3.0 label="IOR" on_change={set_ior} />
                    </List>
                }}
            </Show>
            <Show condition={light}>
                <Slider
                    value={intensity}
                    min=0.1
                    max=8.0
                    label="Intensity"
                    on_change={set_intensity}
                />
            </Show>
            <Button
                label="Delete entity"
                variant=ButtonVariant::Secondary
                @test_id={"pixel_ray_tracer.delete-entity"}
                on_click={move || state.delete_selected()}
            />
        </List>
    }
}

#[component]
fn NewEntity(state: RayState) -> NodeId {
    let tool = state.tool;
    let light = create_memo(move || tool.get() == Tool::Light);
    let surface = create_memo(move || tool.get() == Tool::Surface);
    let intensity = state.new_light_intensity;
    let held = state.new_surface;
    let roughness = create_memo(move || held.get().roughness);
    let metalness = create_memo(move || held.get().metalness);
    let transmission = create_memo(move || held.get().transmission);
    let ior = create_memo(move || held.get().refractive_index);
    let edit = move |held: &block_editor_beui::beui::reactive::ReadSignal<Surface>,
                     write: fn(&mut Surface, f32)| {
        let held = *held;
        move |value: f32| {
            let mut surface = held.get_untracked();
            write(&mut surface, value);
            state.set_new_surface(surface);
        }
    };
    let set_roughness = edit(&held, |surface, value| surface.roughness = value);
    let set_metalness = edit(&held, |surface, value| surface.metalness = value);
    let set_transmission = edit(&held, |surface, value| surface.transmission = value);
    let set_ior = edit(&held, |surface, value| surface.refractive_index = value);
    view! {
        <List spacing=8.0>
            <Show condition={light}>
                {move || view! {
                    <List spacing=6.0>
                        <Heading content="New light" />
                        <Slider
                            value={intensity}
                            min=0.1
                            max=8.0
                            label="Intensity"
                            on_change={move |value: f32| state.set_light_intensity(value)}
                        />
                    </List>
                }}
            </Show>
            <Show condition={surface}>
                {move || view! {
                    <List spacing=6.0>
                        <Heading content="New surface" />
                        <Slider value={roughness} label="Roughness" on_change={set_roughness} />
                        <Slider value={metalness} label="Metalness" on_change={set_metalness} />
                        <Slider
                            value={transmission}
                            label="Transmission"
                            on_change={set_transmission}
                        />
                        <Slider value={ior} min=1.0 max=3.0 label="IOR" on_change={set_ior} />
                    </List>
                }}
            </Show>
        </List>
    }
}

#[component]
fn RaySettingsPanels(state: RayState) -> NodeId {
    let tool = state.tool;
    let tracing = create_memo(move || tool.get() == Tool::RayTrace);
    view! {
        <List spacing=10.0>
            <Show condition={tracing}>
                {move || view! {
                    <List spacing=6.0>
                        <Heading content="View rays" />
                        <RayControls state={state} view=true />
                    </List>
                }}
            </Show>
            <List spacing=6.0>
                <Heading content="Lighting rays" />
                <RayControls state={state} view=false />
            </List>
        </List>
    }
}

#[component]
fn RayControls(state: RayState, view: bool) -> NodeId {
    let entities = state.entities;
    let settings = create_memo(move || {
        let _ = entities.get();
        state.settings(view)
    });
    let count = create_memo(move || settings.get().ray_count as f32);
    let step = create_memo(move || settings.get().step_distance);
    let steps = create_memo(move || settings.get().maximum_steps as f32);
    let write = move |settings: &block_editor_beui::beui::reactive::Memo<RaySettings>,
                      edit: fn(&mut RaySettings, f32)| {
        let settings = *settings;
        move |value: f32| {
            let mut next = settings.get_untracked();
            edit(&mut next, value);
            state.set_settings(view, next);
        }
    };
    let set_count = write(&settings, |settings, value| {
        settings.ray_count = value.round().max(1.0) as u16;
    });
    let set_step = write(&settings, |settings, value| {
        settings.step_distance = value;
    });
    let set_steps = write(&settings, |settings, value| {
        settings.maximum_steps = value.round().max(1.0) as u16;
    });
    view! {
        <List spacing=6.0>
            <Slider value={count} min=1.0 max=2048.0 label="Rays" on_change={set_count} />
            <Slider
                value={step}
                min=0.05
                max=128.0
                scale={SliderScale::Midpoint(4.0)}
                label="Step"
                on_change={set_step}
            />
            <Slider value={steps} min=1.0 max=2048.0 label="Steps" on_change={set_steps} />
        </List>
    }
}
