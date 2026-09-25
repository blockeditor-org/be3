use block_editor_plugin::be_block::{BlockContent, ImageContent};
use std::cell::RefCell;
use std::rc::Rc;

use block_editor_plugin::be_block::pixel_art::PixelColor;
use block_editor_plugin::be_block::pixel_art::{default_palette, size_of};
use block_editor_plugin::be_block::{ObjectId, PixelArtContent, PixelArtDocument};
use block_editor_plugin::beui::reactive::{
    Direction, ItemSize, List, Picture, clone, component, create_effect, create_memo,
    create_signal, view,
};
use block_editor_plugin::beui::{ImageFit, NodeId, Vec2};
use block_editor_plugin::{
    ArtifactDescription, Artifacts, BlockParent, Creation, Editor, Side, Sidebar,
};

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
        Ok(creation.create(&PixelArtContent::new(&PixelArtDocument::new())))
    }

    fn connect_artifact(artifacts: &Artifacts) {
        let regeneration: Rc<RefCell<Option<artifact::Regeneration>>> = Rc::new(RefCell::default());
        let failure: Rc<RefCell<Option<String>>> = Rc::new(RefCell::default());
        let host = artifacts.host().clone();
        let block_id = artifacts.block_id();
        let block_type = artifacts.block_type();
        let started = Rc::clone(&regeneration);
        let reported = Rc::clone(&failure);
        artifacts.on_regenerate(move |data| {
            match artifact::Regeneration::start(&host, block_id, block_type, data) {
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
    let block = editor.block_content::<PixelArtContent>();
    let tools = Tools::new(&editor, Rc::clone(&block));
    let pane = Pane::new();
    let shown = pane.shown();
    let size = block.project(size_of);
    let size = create_memo(clone!(size -> move || size.get()));
    let palette = block.project(|art| {
        art.field(ObjectId::ROOT, PixelArtDocument::PALETTE)
            .unwrap_or_else(default_palette)
    });
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
    let block = editor.block_content::<PixelArtContent>();
    let pane = Pane::new();
    let shown = pane.shown();
    let refreshed = Rc::clone(&pane);
    let watched = Rc::clone(&block);
    create_effect(move || refreshed.refresh(&watched, true, &[], PixelColor::TRANSPARENT));
    let image = create_memo(clone!(shown -> move || shown.get().artwork));
    view! {
        <Picture image={image} fit=ImageFit::Contain smooth=false />
    }
}

pub(crate) fn export(tools: &Rc<Tools>) {
    let editor = tools.editor();
    let name = editor
        .blocks()
        .info(editor.block_id())
        .and_then(|info| info.name)
        .unwrap_or_else(|| "Pixel Art".to_owned());
    let Some(art) = tools.artwork() else {
        return;
    };
    let generated = artifact::generate_initial(&art, &name);
    match generated {
        Ok(image) => {
            let child = editor.blocks().create_artifact(
                &image,
                BlockParent::Detached,
                None,
                artifact::descriptor(editor.block_id()),
            );
            editor.host().open_block(child, ImageContent::CONTENT_TYPE);
            tools.set_export_error.set(None);
        }
        Err(error) => tools.set_export_error.set(Some(error)),
    }
}
