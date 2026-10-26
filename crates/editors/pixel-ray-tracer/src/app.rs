use block_editor_beui::Fit;
use pixel_ray_tracer_types::PixelRayTracerContent;

use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{ImageFit, NodeId, Vec2};
use block_editor_beui::{Creation, Editor, Fitting, Side, Sidebar, Toolbar};
use pixel_ray_tracer_types::PIXEL_RAY_TRACER_SIZE;
use uuid::Uuid;

pub(crate) mod canvas;
pub(crate) mod panels;
pub(crate) mod state;

use canvas::Artwork;
use panels::{PropertiesPanel, ToolsPanel};
use state::RayState;

const TOOLS_WIDTH: Size = Size::Custom(148.0);

pub struct PixelRayTracerApp;

#[component]
impl block_editor_beui::BeuiApp for PixelRayTracerApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <PixelRayTracerEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <PixelRayTracerPreview editor={editor} />
            </Loaded>
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
    let content = NodeRef::new();
    editor.content(&content);
    let fitting = editor.keep_fitted(|| Some(Fit::World));

    let chrome = editor.chrome_shown();
    view! {
        <List spacing=Space::None>
            <RayToolbar state={state} fitting={fitting} shown={chrome} />
            <List
                @sizing=ItemSize::Percent(100.0)
                direction=Direction::Horizontal
                spacing=Space::None
            >
                <Sidebar side=Side::Left shown={chrome} width=TOOLS_WIDTH>
                    <ToolsPanel state={state} />
                </Sidebar>
                <Artwork @sizing=ItemSize::Percent(100.0) @node_ref={&content} state={state} />
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
fn RayToolbar(
    state: RayState,
    fitting: Fitting,
    shown: block_editor_beui::beui::reactive::Prop<bool>,
) -> DynamicSegment<ListChild> {
    let size = format!("{PIXEL_RAY_TRACER_SIZE} × {PIXEL_RAY_TRACER_SIZE}");
    view! {
        <Toolbar shown={shown}>
            <Text>"Pixel Ray Tracer"</Text>
            <Text style=TextStyle::CAPTION>{size}</Text>
            <Button
                label="Fit view"
                variant=ButtonVariant::Secondary
                @test_id={"pixel_ray_tracer.fit"}
                on_click={move || fitting.fit()}
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
