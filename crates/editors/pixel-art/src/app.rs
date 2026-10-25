use block_editor_beui::Loaded;
use block_editor_beui::be_block::BlockContent;
use std::cell::RefCell;
use std::rc::Rc;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::Picture;
use block_editor_beui::beui::{ImageFit, NodeId, Vec2};
use block_editor_beui::{Artifacts, BlockParent, Creation, Editor, Side, Sidebar};
use pixel_art_types::PixelColor;
use pixel_art_types::{PixelArtContent, PixelArtDocument};
use pixel_art_types::{default_palette, size_of};

use crate::artifact;
use crate::artifact::Settings;
use crate::panels::{ColorsPanel, Dialogs, ToolsPanel, TopBar};

pub(crate) mod canvas;
pub(crate) mod pane;
pub(crate) mod state;

use canvas::{ArtworkCanvas, HoverLabel};
use image_block_types::ImageContent;
use pane::Pane;
use state::Tools;

const EMBEDDED_LONG_SIDE: f32 = 256.0;
const EMBEDDED_SHORT_SIDE: f32 = 24.0;
const TOOLS_WIDTH: f32 = 180.0;

pub struct PixelArtApp;

#[component]
impl block_editor_beui::BeuiApp for PixelArtApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <PixelArtEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <PixelArtPreview editor={editor} />
            </Loaded>
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
        let source = artifacts.source();
        let started = Rc::clone(&regeneration);
        let reported = Rc::clone(&failure);
        artifacts.on_regenerate(move |data| {
            match artifact::Regeneration::start(&host, block_id, block_type, source, data) {
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

    fn describe_artifact(data: &[u8]) -> Result<String, String> {
        artifact::describe(data)
    }

    #[component]
    fn ArtifactSettings(artifacts: Artifacts) -> NodeId {
        view! {
            <Settings artifacts={artifacts} />
        }
    }
}

#[component]
fn PixelArtEditor(editor: Editor) -> NodeId {
    let block = editor.block_content::<PixelArtContent>();
    let tools = Tools::new(editor, block);
    let pane = Pane::new();
    let shown = pane.shown();
    let size = block.project(size_of);
    let size = create_memo(move || size.get());
    let palette = block.project(|art| {
        art.field(ObjectId::ROOT, PixelArtDocument::PALETTE)
            .unwrap_or_else(default_palette)
    });
    let palette = create_memo(move || palette.get());
    let hovered = create_signal(None::<(u16, u16)>);

    create_effect(move || {
        let (width, height) = size.get();
        if width == 0 || height == 0 {
            return;
        }
        let width = f32::from(width);
        let height = f32::from(height);
        let pixel =
            (EMBEDDED_LONG_SIDE / width.max(height)).max(EMBEDDED_SHORT_SIDE / width.min(height));
        editor.set_intrinsic_size(Some(Vec2::new(width * pixel, height * pixel)));
    });

    let chrome = editor.chrome_shown();
    view! {
        <List spacing=0.0>
            <TopBar tools={tools} size={size} shown={chrome} />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <Sidebar side=Side::Left shown={chrome} width=TOOLS_WIDTH>
                    <ToolsPanel tools={tools} />
                </Sidebar>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <ArtworkCanvas
                        @sizing=ItemSize::Percent(100.0)
                        tools={tools}
                        pane={pane}
                        shown={shown}
                        size={size}
                        hovered={hovered}
                    />
                    <HoverLabel tools={tools} hovered={hovered.read_only()} />
                </List>
                <Sidebar shown={chrome}>
                    <ColorsPanel tools={tools} palette={palette} />
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
    create_effect(move || pane.refresh(&block, true, &[], PixelColor::TRANSPARENT));
    let image = create_memo(move || shown.get().artwork);
    view! {
        <Picture image={image} fit=ImageFit::Contain smooth=false />
    }
}

pub(crate) fn export(tools: Tools) {
    let editor = tools.editor();
    let name = editor
        .blocks()
        .info(editor.block_id())
        .and_then(|info| info.name())
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
            tools.export_error.set(None);
        }
        Err(error) => tools.export_error.set(Some(error)),
    }
}
