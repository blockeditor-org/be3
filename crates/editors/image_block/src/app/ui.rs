use std::rc::Rc;

use block_editor_plugin::be_block::ImageContent;
use block_editor_plugin::beui::reactive::{
    Align, Canvas, CanvasItem, Direction, Frame, ItemSize, List, Memo, NodeRef, Picture, Show,
    Spacer, clone, component, component_rect, create_effect, create_memo, view,
};
use block_editor_plugin::beui::styled::{Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_plugin::beui::{ImageFit, NodeId, Pos2, Rect, Vec2};
use block_editor_plugin::{Editor, FileChooser, Sidebar, fit_content};

use super::picture::watch;
use super::{filter, imported};

#[component]
pub fn ImageEditor(editor: Editor) -> NodeId {
    let block = editor.block_content::<ImageContent>();
    let shown = watch(&editor, &block);
    let image = create_memo(clone!(shown -> move || shown.get().image));
    let failed = create_memo(clone!(shown -> move || shown.get().error.is_some()));
    let reason = create_memo(clone!(shown -> move || shown.get().error.unwrap_or_default()));

    let chooser = FileChooser::new(filter(), imported);
    let polled = Rc::clone(&chooser);
    let host = editor.host().clone();
    let replacing = editor.clone();
    editor.each_frame(move || {
        polled.poll(&host);
        if let Some(image) = polled.take() {
            replacing.replace_content(replacing.block_id(), &image);
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(clone!(read_only busy -> move || read_only.get() || busy.get()));
    let failure = chooser.error();
    let refused = create_memo(clone!(failure -> move || failure.get().is_some()));
    let refusal = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let opening = editor.host().clone();
    let choose = clone!(chooser -> move || chooser.open(&opening));

    let sized = editor.clone();
    create_effect(clone!(image -> move || {
        sized.set_intrinsic_size(image.get().map(|image| image.size()));
    }));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    let danger = theme.danger.clone();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <List align=Align::Stretch spacing=0.0>
                    <Artwork @sizing=ItemSize::Percent(100.0) editor={editor} image={image} />
                    <Show condition={failed}>
                        <Caption content={reason} color={danger} />
                    </Show>
                </List>
            </Frame>
            <Sidebar shown={chrome}>
                <Heading content="Image" />
                <Button
                    label="Replace image…"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={"image.replace"}
                    on_click={choose}
                />
                <Show condition={refused}>
                    <Caption content={refusal} color={theme.danger.clone()} />
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </Sidebar>
        </List>
    }
}

#[component]
fn Artwork(editor: Editor, image: Memo<Option<block_editor_plugin::beui::Image>>) -> NodeId {
    let placed = component_rect();
    let world = editor.world();
    let shape = create_memo(clone!(image world placed -> move || {
        let Some(size) = image.get().map(|image| image.size()) else {
            return Rect::ZERO;
        };
        let available = world.get().unwrap_or_else(|| placed.get().size());
        fit_content(
            Rect::from_min_size(Pos2::ZERO, available.max(Vec2::new(1.0, 1.0))),
            size,
        )
    }));
    let x = create_memo(clone!(shape -> move || shape.get().left()));
    let y = create_memo(clone!(shape -> move || shape.get().top()));
    let width = create_memo(clone!(shape -> move || shape.get().width()));
    let height = create_memo(clone!(shape -> move || shape.get().height()));
    view! {
        <Canvas view={editor.canvas()}>
            <CanvasItem x={x} y={y} width={width} height={height} @test_id={"image.picture"}>
                <Picture image={image} fit=ImageFit::Fill />
            </CanvasItem>
        </Canvas>
    }
}

#[component]
pub fn ImagePreview(editor: Editor) -> NodeId {
    let block = editor.block_content::<ImageContent>();
    let shown = watch(&editor, &block);
    let image = create_memo(move || shown.get().image);
    view! {
        <Picture image={image} fit=ImageFit::Contain />
    }
}
