use block_editor_beui::be_block::ImageContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::Picture;
use block_editor_beui::beui::{Image, ImageFit, NodeId, Pos2, Rect, Thumbhash, Vec2};
use block_editor_beui::{Editor, FileChooser, Sidebar, fit_content};

use super::picture::{thumbhash, watch};
use super::{filter, imported};

#[component]
pub fn ImageEditor(editor: Editor) -> NodeId {
    let block = editor.block_content::<ImageContent>();
    let shown = watch(editor, block);
    let image = create_memo(move || shown.get().image);
    let placeholder = thumbhash(editor);
    let failed = create_memo(move || shown.get().error.is_some());
    let reason = create_memo(move || shown.get().error.unwrap_or_default());

    let chooser = FileChooser::new(filter(), imported);
    chooser.on_chosen(move |chooser| {
        if let Some(image) = chooser.take() {
            editor.replace_content(editor.block_id(), &image);
        }
    });

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(move || read_only.get() || busy.get());
    let failure = chooser.error();
    let refused = create_memo(move || failure.get().is_some());
    let refusal = create_memo(move || failure.get().unwrap_or_default());
    let choose = move || chooser.open();

    create_effect(move || {
        editor.set_intrinsic_size(image.get().map(|image| image.size()));
    });

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <List align=Align::Stretch spacing=0.0>
                    <Artwork
                        @sizing=ItemSize::Percent(100.0)
                        editor={editor}
                        image={image}
                        thumbhash={placeholder}
                    />
                    <Show condition={failed}>
                        <Text style=TextStyle::CAPTION string={reason} color=TextColor::Danger />
                    </Show>
                </List>
            </Frame>
            <Sidebar shown={chrome}>
                <Text style=TextStyle::HEADING string="Image" />
                <Button
                    label="Replace image…"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={"image.replace"}
                    on_click={choose}
                />
                <Show condition={refused}>
                    <Text style=TextStyle::CAPTION string={refusal} color=TextColor::Danger />
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </Sidebar>
        </List>
    }
}

#[component]
fn Artwork(
    editor: Editor,
    image: Memo<Option<Image>>,
    thumbhash: Memo<Option<Thumbhash>>,
) -> NodeId {
    view! {
        <PlacementBuilder>
            |placed: ReadSignal<Rect>| {
                let world = editor.world();
                let placeholder = create_memo(move || {
                    thumbhash.get().map(|thumbhash| thumbhash.size())
                });
                let shape = create_memo(move || {
                    let Some(size) = image.get().map(|image| image.size()).or_else(|| placeholder.get()) else {
                        return Rect::ZERO;
                    };
                    let available = world.get().unwrap_or_else(|| placed.get().size());
                    fit_content(
                        Rect::from_min_size(Pos2::ZERO, available.max(Vec2::new(1.0, 1.0))),
                        size,
                    )
                });
                let x = create_memo(move || shape.get().left());
                let y = create_memo(move || shape.get().top());
                let width = create_memo(move || shape.get().width());
                let height = create_memo(move || shape.get().height());
                view! {
                    <Canvas view={editor.canvas()}>
                        <CanvasItem
                            x={x}
                            y={y}
                            width={width}
                            height={height}
                            @test_id={"image.picture"}
                        >
                            <Picture image={image} thumbhash={thumbhash} fit=ImageFit::Fill />
                        </CanvasItem>
                    </Canvas>
                }
            }
        </PlacementBuilder>
    }
}

#[component]
pub fn ImagePreview(editor: Editor) -> NodeId {
    let block = editor.block_content::<ImageContent>();
    let shown = watch(editor, block);
    let image = create_memo(move || shown.get().image);
    let placeholder = thumbhash(editor);
    view! {
        <Picture image={image} thumbhash={placeholder} fit=ImageFit::Contain />
    }
}
