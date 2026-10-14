use block_editor_beui::be_block::PixelRayTracerContent;

use block_editor_beui::be_block::pixel_ray_tracer::PIXEL_RAY_TRACER_SIZE;
use block_editor_beui::beui::reactive::{Direction, ItemSize, List, component, view};
use block_editor_beui::beui::styled::{Button, ButtonVariant};
use block_editor_beui::beui::unstyled::Picture;
use block_editor_beui::beui::{ImageFit, NodeId, Vec2};
use block_editor_beui::{Creation, Editor, Side, Sidebar, Toolbar};
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod panels;
pub(crate) mod state;

use block_editor_beui::beui::styled::{Text, TextStyle};
use canvas::Artwork;
use panels::{PropertiesPanel, ToolsPanel};
use state::RayState;

const TOOLS_WIDTH: f32 = 148.0;

pub struct PixelRayTracerApp;

impl block_editor_beui::BeuiApp for PixelRayTracerApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PixelRayTracerEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <PixelRayTracerPreview editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&PixelRayTracerContent::default()))
    }

    fn aspect_ratio() -> Option<f32> {
        Some(1.0)
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::splat(384.0))
    }
}

#[component]
fn PixelRayTracerEditor(editor: Editor) -> NodeId {
    let state = RayState::new(editor);
    state.watch();

    let chrome = editor.chrome_shown();
    view! {
        <List spacing=0.0>
            <RayToolbar state={state} shown={chrome} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <Sidebar side=Side::Left shown={chrome} width=TOOLS_WIDTH>
                    <ToolsPanel state={state} />
                </Sidebar>
                <Artwork @sizing=ItemSize::Percent(100.0) state={state} />
                <Sidebar shown={chrome}>
                    <PropertiesPanel state={state} />
                </Sidebar>
            </List>
        </List>
    }
}

#[component]
fn PixelRayTracerPreview(editor: Editor) -> NodeId {
    let state = RayState::new(editor);
    state.watch();
    let image = state.lighting;
    view! {
        <Picture image={image} fit=ImageFit::Contain smooth=false />
    }
}

#[component]
fn RayToolbar(state: RayState, shown: block_editor_beui::beui::reactive::Prop<bool>) -> NodeId {
    let fit = state.editor();
    let size = format!("{PIXEL_RAY_TRACER_SIZE} × {PIXEL_RAY_TRACER_SIZE}");
    view! {
        <Toolbar shown={shown}>
            <Text string="Pixel Ray Tracer" />
            <Text style=TextStyle::CAPTION string={size} />
            <Button
                label="Fit view"
                variant=ButtonVariant::Secondary
                @test_id={"pixel_ray_tracer.fit"}
                on_click={move || fit.fit()}
            />
            <Button
                label="Reset artwork"
                variant=ButtonVariant::Secondary
                @test_id={"pixel_ray_tracer.reset"}
                on_click={move || state.reset()}
            />
        </Toolbar>
    }
}
