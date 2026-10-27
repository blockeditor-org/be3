use block_editor_beui::Fit;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Image, ImageFit, NodeId, Pos2, Rect, Thumbhash, Vec2};
use block_editor_beui::{Editor, FileChooser, Sidebar, fit_content};

use super::picture::{thumbhash, watch};
use super::{filter, imported};
use beui::{Align, Direction, ItemSize};
use image_block_types::ImageContent;

#[component]
pub fn ImageEditor(editor: Editor) -> NodeId {
    let placeholder = thumbhash(editor);

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

    let fallback = move |pending: Pending| {
        let image = create_memo(|| None);
        let error = match pending {
            Pending::Loading => None,
            Pending::Failed(error) => Some(error),
        };
        let error = create_memo(move || error.clone());
        view! {
            <ImageArea editor={editor} image={image} thumbhash={placeholder} error={error} />
        }
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    editor.keep_fitted(|| Some(Fit::World));
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                <Layers>
                    <Loading fallback>|| {
                        let block = editor.block_content::<ImageContent>();
                        let shown = watch(editor, block);
                        let image = create_memo(move || shown.get().image);
                        let error = create_memo(move || shown.get().error);
                        create_effect(move || {
                            editor.set_intrinsic_size(image.get().map(|image| image.size()));
                        });
                        view! {
                            <ImageArea
                                editor={editor}
                                image={image}
                                thumbhash={placeholder}
                                error={error}
                            />
                        }
                    }</Loading>
                </Layers>
            </Frame>
            <Sidebar shown={chrome}>
                <Text style=TextStyle::HEADING>"Image"</Text>
                <Button
                    label="Replace image…"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={"image.replace"}
                    on_click={choose}
                />
                <Show condition={refused}>
                    <Text style=TextStyle::CAPTION color=TextColor::Danger>{refusal}</Text>
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </Sidebar>
        </List>
    }
}

#[component]
fn ImageArea(
    editor: Editor,
    image: Memo<Option<Image>>,
    thumbhash: Memo<Option<Thumbhash>>,
    error: Memo<Option<String>>,
) -> NodeId {
    let failed = create_memo(move || error.with(Option::is_some));
    let reason = create_memo(move || error.get().unwrap_or_default());
    view! {
        <List align=Align::Stretch spacing=Space::None>
            <Artwork
                @sizing=ItemSize::Percent(100.0)
                editor={editor}
                image={image}
                thumbhash={thumbhash}
            />
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
            </Show>
        </List>
    }
}

#[component]
fn Artwork(
    editor: Editor,
    image: Memo<Option<Image>>,
    thumbhash: Memo<Option<Thumbhash>>,
) -> NodeId {
    let placed_held = create_signal(Rect::ZERO);
    let placed = placed_held.read_only();
    let world = editor.world();
    let placeholder = create_memo(move || thumbhash.get().map(|thumbhash| thumbhash.size()));
    let shape = create_memo(move || {
        let Some(size) = image
            .get()
            .map(|image| image.size())
            .or_else(|| placeholder.get())
        else {
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
    let built = view! {
        <Canvas view={editor.canvas()}>
            <CanvasItem x={x} y={y} width={width} height={height} @test_id={"image.picture"}>
                <Picture image={image} thumbhash={thumbhash} fit=ImageFit::Fill />
            </CanvasItem>
        </Canvas>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placed_measured.get() {
            placed_held.set(measured.rect);
        }
    });
    built
}

#[component]
pub fn ImagePreview(editor: Editor) -> NodeId {
    let placeholder = thumbhash(editor);
    let fallback = move |_: Pending| {
        let image = create_memo(|| None);
        view! {
            <Picture image={image} thumbhash={placeholder} fit=ImageFit::Contain />
        }
    };
    view! {
        <Layers>
            <Loading fallback>|| {
                let block = editor.block_content::<ImageContent>();
                let shown = watch(editor, block);
                let image = create_memo(move || shown.get().image);
                view! {
                    <Picture image={image} thumbhash={placeholder} fit=ImageFit::Contain />
                }
            }</Loading>
        </Layers>
    }
}
