use std::cell::RefCell;
use std::rc::Rc;

use block::Block;
use block_client::blocks::image::Image;
use block_client::blocks::pixel_art::{PixelArt, PixelColor};
use block_editor_plugin::beui::reactive::{
    Direction, ItemSize, List, Picture, clone, component, create_effect, create_memo,
    create_signal, view,
};
use block_editor_plugin::beui::{ImageFit, NodeId, Vec2};
use block_editor_plugin::{ArtifactDescription, Artifacts, Creation, Editor, Side, Sidebar};

use crate::artifact;
use crate::artifact::Settings;
use crate::panels::{ColorsPanel, Dialogs, ToolsPanel, TopBar};

pub(crate) mod canvas;
pub(crate) mod pane;
pub(crate) mod state;

use canvas::{ArtworkCanvas, HoverLabel};
use pane::Pane;
use state::Tools;

const EMBEDDED_LONG_SIDE: f32 = 256.0;
const EMBEDDED_SHORT_SIDE: f32 = 24.0;
const TOOLS_WIDTH: f32 = 180.0;

pub struct PixelArtApp;

impl block_editor_plugin::BeuiApp for PixelArtApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PixelArtEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <PixelArtPreview editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        Ok(creation.client().create_block(PixelArt::new()).id())
    }

    fn connect_artifact(artifacts: &Artifacts) {
        let regeneration: Rc<RefCell<Option<artifact::Regeneration>>> = Rc::new(RefCell::default());
        let failure: Rc<RefCell<Option<String>>> = Rc::new(RefCell::default());
        let client = artifacts.client().clone();
        let host = artifacts.host().clone();
        let block_id = artifacts.block_id();
        let block_type = artifacts.block_type();
        let started = Rc::clone(&regeneration);
        let reported = Rc::clone(&failure);
        artifacts.on_regenerate(move |data| {
            match artifact::Regeneration::start(&host, &client, block_id, block_type, data) {
                Ok(started_regeneration) => {
                    *started.borrow_mut() = Some(started_regeneration);
                    reported.borrow_mut().take();
                }
                Err(error) => {
                    started.borrow_mut().take();
                    *reported.borrow_mut() = Some(error);
                }
            }
        });
        artifacts.on_poll(move || {
            if let Some(error) = failure.borrow_mut().take() {
                return Some(Err(error));
            }
            let result = regeneration.borrow_mut().as_mut()?.poll()?;
            regeneration.borrow_mut().take();
            Some(result)
        });
    }

    fn describe_artifact(data: &[u8]) -> Result<ArtifactDescription, String> {
        artifact::describe(data)
    }

    fn artifact_settings_view(artifacts: Artifacts) -> NodeId {
        view! {
            <Settings artifacts={artifacts} />
        }
    }
}

#[component]
fn PixelArtEditor(editor: Editor) -> NodeId {
    let block = editor.block::<PixelArt>();
    let tools = Tools::new(&editor, Rc::clone(&block));
    let pane = Pane::new();
    let shown = pane.shown();
    let size = block.project(|art| (art.width(), art.height()));
    let size = create_memo(clone!(size -> move || size.get()));
    let palette = block.project(|art| art.palette().to_vec());
    let palette = create_memo(clone!(palette -> move || palette.get()));
    let (hovered, set_hovered) = create_signal(None::<(u16, u16)>);

    let sized = editor.clone();
    create_effect(clone!(size -> move || {
        let (width, height) = size.get();
        if width == 0 || height == 0 {
            return;
        }
        let width = f32::from(width);
        let height = f32::from(height);
        let pixel =
            (EMBEDDED_LONG_SIDE / width.max(height)).max(EMBEDDED_SHORT_SIDE / width.min(height));
        sized.set_intrinsic_size(Some(Vec2::new(width * pixel, height * pixel)));
    }));

    let chrome = editor.chrome_shown();
    let canvas_size = size.clone();
    let bar_tools = Rc::clone(&tools);
    let left_tools = Rc::clone(&tools);
    let canvas_tools = Rc::clone(&tools);
    let label_tools = Rc::clone(&tools);
    let colors_tools = Rc::clone(&tools);
    let top_chrome = chrome.clone();
    let left_chrome = chrome.clone();
    view! {
        <List spacing=0.0>
            <TopBar tools={bar_tools} size={size.clone()} shown={top_chrome} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <Sidebar side=Side::Left shown={left_chrome} width=TOOLS_WIDTH>
                    <ToolsPanel tools={left_tools} />
                </Sidebar>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <ArtworkCanvas
                        @sizing=ItemSize::Percent(100.0)
                        tools={canvas_tools}
                        pane={pane}
                        shown={shown}
                        size={canvas_size}
                        hovered={hovered.clone()}
                        set_hovered={set_hovered}
                    />
                    <HoverLabel tools={label_tools} hovered={hovered} />
                </List>
                <Sidebar shown={chrome}>
                    <ColorsPanel tools={colors_tools} palette={palette} />
                </Sidebar>
            </List>
            <Dialogs tools={tools} size={size} />
        </List>
    }
}

#[component]
fn PixelArtPreview(editor: Editor) -> NodeId {
    let block = editor.block::<PixelArt>();
    let pane = Pane::new();
    let shown = pane.shown();
    let refreshed = Rc::clone(&pane);
    let watched = Rc::clone(&block);
    let frame = editor.clone();
    editor.each_frame(move || {
        refreshed.refresh(&frame, &watched, true, &[], PixelColor::TRANSPARENT);
    });
    let image = create_memo(clone!(shown -> move || shown.get().artwork));
    view! {
        <Picture image={image} fit=ImageFit::Contain smooth=false />
    }
}

pub(crate) fn export(tools: &Rc<Tools>) {
    let editor = tools.editor();
    let handle = tools.block().handle();
    let name = handle.name().unwrap_or_else(|| "Pixel Art".to_owned());
    let Some(art) = handle.read() else {
        return;
    };
    let generated = artifact::generate_initial(&art, &name);
    drop(art);
    match generated {
        Ok(image) => {
            let child = editor
                .client()
                .create_dynamic_artifact(Image::new(), artifact::descriptor(handle.id()));
            editor.seed_content(child.id(), &image);
            editor.host().open_block(child.id(), Image::TYPE_ID);
            tools.set_export_error.set(None);
        }
        Err(error) => tools.set_export_error.set(Some(error)),
    }
}
