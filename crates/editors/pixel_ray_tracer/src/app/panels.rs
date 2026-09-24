use std::rc::Rc;

use block_editor_plugin::be_block::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_PALETTE, RayEntity, RaySettings,
};
use block_editor_plugin::beui::Color32;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::accesskit::{Node, Role};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, List, Prop, Show, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, Slider, ToggleButton, Tooltip, use_theme,
};
use block_editor_plugin::beui::unstyled::{Pressable, SliderScale};

use crate::overlay::palette_color;

use super::state::{RayState, Surface, Tool};

const SWATCH: f32 = 18.0;

#[component]
pub(crate) fn ToolsPanel(state: Rc<RayState>) -> NodeId {
    let theme = use_theme();
    view! {
        <List spacing=8.0>
            <Heading content="Tools" />
            <ForEach keys={Tool::ALL.to_vec()}>
                {move |tool: Tool| {
                    let state = Rc::clone(&state);
                    view! {
                        <ToolButton state tool />
                    }
                }}
            </ForEach>
            <Caption
                content="Scroll: pan · Ctrl scroll: zoom · Middle drag: pan · Delete: remove entity"
                color={theme.text_muted.clone()}
            />
        </List>
    }
}

#[component]
fn ToolButton(state: Rc<RayState>, tool: Tool) -> NodeId {
    let chosen = state.tool.clone();
    let pressed = create_memo(clone!(chosen -> move || chosen.get() == tool));
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
pub(crate) fn PropertiesPanel(state: Rc<RayState>) -> NodeId {
    let palette = Rc::clone(&state);
    let entity = Rc::clone(&state);
    let tools = Rc::clone(&state);
    let rays = Rc::clone(&state);
    let selected = create_memo(clone!(entity -> move || entity.selected_entity()));
    let chosen = create_memo(clone!(selected -> move || selected.get().is_some()));
    let unchosen = create_memo(clone!(chosen -> move || !chosen.get()));
    view! {
        <List spacing=10.0>
            <Heading content="Palette" />
            <Palette state={palette} />
            <Show condition={chosen}>
                <SelectedEntity state={Rc::clone(&state)} selected={selected} />
            </Show>
            <Show condition={unchosen}>
                <NewEntity state={tools} />
            </Show>
            <RaySettingsPanels state={rays} />
        </List>
    }
}

#[component]
fn Palette(state: Rc<RayState>) -> NodeId {
    let indexes = (0..PIXEL_RAY_TRACER_PALETTE.len()).collect::<Vec<usize>>();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
            <ForEach keys={indexes}>
                {move |index: usize| {
                    let state = Rc::clone(&state);
                    view! {
                        <Swatch state index />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn Swatch(state: Rc<RayState>, index: usize) -> NodeId {
    let color = palette_color(index as u8);
    let chosen = state.color_index.clone();
    let picked = create_memo(clone!(chosen -> move || usize::from(chosen.get()) == index));
    let theme = use_theme();
    let outline = create_memo(clone!(theme picked -> move || match picked.get() {
        true => theme.accent.get(),
        false => Color32::TRANSPARENT,
    }));
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
    state: Rc<RayState>,
    selected: block_editor_plugin::beui::reactive::Memo<Option<RayEntity>>,
) -> NodeId {
    let kind = create_memo(clone!(selected -> move || match selected.get() {
        Some(RayEntity::Surface { .. }) => "Surface",
        Some(RayEntity::Light { .. }) => "Light",
        Some(RayEntity::Water { .. }) => "Water",
        None => "",
    }
    .to_owned()));
    let surface = create_memo(clone!(selected -> move || {
        matches!(selected.get(), Some(RayEntity::Surface { .. }))
    }));
    let light = create_memo(clone!(selected -> move || {
        matches!(selected.get(), Some(RayEntity::Light { .. }))
    }));
    let field = |read: fn(&RayEntity) -> f32| {
        let selected = selected.clone();
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
    let edit = |state: &Rc<RayState>,
                selected: &block_editor_plugin::beui::reactive::Memo<Option<RayEntity>>,
                write: fn(&mut RayEntity, f32)| {
        let state = Rc::clone(state);
        let selected = selected.clone();
        move |value: f32| {
            let Some(mut entity) = selected.get_untracked() else {
                return;
            };
            write(&mut entity, value);
            state.update_entity(entity);
        }
    };
    let set_intensity = edit(&state, &selected, |entity, value| {
        if let RayEntity::Light { intensity, .. } = entity {
            *intensity = value;
        }
    });
    let set_roughness = edit(&state, &selected, |entity, value| {
        if let RayEntity::Surface { roughness, .. } = entity {
            *roughness = value;
        }
    });
    let set_metalness = edit(&state, &selected, |entity, value| {
        if let RayEntity::Surface { metalness, .. } = entity {
            *metalness = value;
        }
    });
    let set_transmission = edit(&state, &selected, |entity, value| {
        if let RayEntity::Surface { transmission, .. } = entity {
            *transmission = value;
        }
    });
    let set_ior = edit(&state, &selected, |entity, value| {
        if let RayEntity::Surface {
            refractive_index, ..
        } = entity
        {
            *refractive_index = value;
        }
    });
    let removing = Rc::clone(&state);
    view! {
        <List spacing=8.0>
            <Heading content="Selected entity" />
            <Body content={kind} />
            <Show condition={surface.clone()}>
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
                on_click={move || removing.delete_selected()}
            />
        </List>
    }
}

#[component]
fn NewEntity(state: Rc<RayState>) -> NodeId {
    let tool = state.tool.clone();
    let light = create_memo(clone!(tool -> move || tool.get() == Tool::Light));
    let surface = create_memo(clone!(tool -> move || tool.get() == Tool::Surface));
    let intensity = state.new_light_intensity.clone();
    let held = state.new_surface.clone();
    let roughness = create_memo(clone!(held -> move || held.get().roughness));
    let metalness = create_memo(clone!(held -> move || held.get().metalness));
    let transmission = create_memo(clone!(held -> move || held.get().transmission));
    let ior = create_memo(clone!(held -> move || held.get().refractive_index));
    let lit = Rc::clone(&state);
    let edit = |state: &Rc<RayState>,
                held: &block_editor_plugin::beui::reactive::ReadSignal<Surface>,
                write: fn(&mut Surface, f32)| {
        let state = Rc::clone(state);
        let held = held.clone();
        move |value: f32| {
            let mut surface = held.get_untracked();
            write(&mut surface, value);
            state.set_new_surface(surface);
        }
    };
    let set_roughness = edit(&state, &held, |surface, value| surface.roughness = value);
    let set_metalness = edit(&state, &held, |surface, value| surface.metalness = value);
    let set_transmission = edit(&state, &held, |surface, value| surface.transmission = value);
    let set_ior = edit(&state, &held, |surface, value| {
        surface.refractive_index = value
    });
    view! {
        <List spacing=8.0>
            <Show condition={light}>
                <List spacing=6.0>
                    <Heading content="New light" />
                    <Slider
                        value={intensity}
                        min=0.1
                        max=8.0
                        label="Intensity"
                        on_change={move |value: f32| lit.set_light_intensity(value)}
                    />
                </List>
            </Show>
            <Show condition={surface}>
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
            </Show>
        </List>
    }
}

#[component]
fn RaySettingsPanels(state: Rc<RayState>) -> NodeId {
    let tool = state.tool.clone();
    let tracing = create_memo(clone!(tool -> move || tool.get() == Tool::RayTrace));
    let view_rays = Rc::clone(&state);
    let lighting = Rc::clone(&state);
    view! {
        <List spacing=10.0>
            <Show condition={tracing}>
                <List spacing=6.0>
                    <Heading content="View rays" />
                    <RayControls state={view_rays} view=true />
                </List>
            </Show>
            <List spacing=6.0>
                <Heading content="Lighting rays" />
                <RayControls state={lighting} view=false />
            </List>
        </List>
    }
}

#[component]
fn RayControls(state: Rc<RayState>, view: bool) -> NodeId {
    let entities = state.entities.clone();
    let read = Rc::clone(&state);
    let settings = create_memo(clone!(read entities -> move || {
        let _ = entities.get();
        read.settings(view)
    }));
    let count = create_memo(clone!(settings -> move || settings.get().ray_count as f32));
    let step = create_memo(clone!(settings -> move || settings.get().step_distance));
    let steps = create_memo(clone!(settings -> move || settings.get().maximum_steps as f32));
    let write = |state: &Rc<RayState>,
                 settings: &block_editor_plugin::beui::reactive::Memo<RaySettings>,
                 edit: fn(&mut RaySettings, f32)| {
        let state = Rc::clone(state);
        let settings = settings.clone();
        move |value: f32| {
            let mut next = settings.get_untracked();
            edit(&mut next, value);
            state.set_settings(view, next);
        }
    };
    let set_count = write(&state, &settings, |settings, value| {
        settings.ray_count = value.round().max(1.0) as u16;
    });
    let set_step = write(&state, &settings, |settings, value| {
        settings.step_distance = value;
    });
    let set_steps = write(&state, &settings, |settings, value| {
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
