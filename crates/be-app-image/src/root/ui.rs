use be_sdk_beui::Fit;
use be_sdk_beui::beui::dock::{DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Image, ImageFit, NodeId, Pos2, Rect, Thumbhash, Vec2};
use be_sdk_beui::{Editor, FileChooser, SIDE_PANEL_WIDTH, fit_content};

use super::picture::{thumbhash, watch};
use super::{filter, imported};
use be_type_image::ImageContent;
use beui::Align;

const STAGE: &str = "stage";
const PANEL: &str = "panel";
const PANEL_WIDTH: f32 = SIDE_PANEL_WIDTH;
const PANEL_PADDING: Space = Space::Lg;
const PANEL_SPACING: Space = Space::Md;

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
    let failure = chooser.error();
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
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="image" size={SplitSize::Second(PANEL_WIDTH)}>
                <DockPane id="stage" locked=true bare=true>
                    <DockTab id=STAGE title="Picture">
                        <Frame @node_ref={&content}>
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
                        </Frame>
                    </DockTab>
                </DockPane>
                <Show value={chrome}>
                    <DockPane id="panel" locked=true>
                        <DockTab id=PANEL title="Image">
                            <Frame fill=ColorRole::Surface>
                                <Scroll @test_id={"image.panel"}>
                                    <Frame padding=PANEL_PADDING>
                                        <List spacing=PANEL_SPACING>
                                            <Button
                                                label="Replace image…"
                                                variant=ButtonVariant::Secondary
                                                disabled={move || read_only.get() || busy.get()}
                                                @test_id={"image.replace"}
                                                on_click={choose}
                                            />
                                            <Show value={failure}>|refusal: ReadSignal<String>|
                                                <Text
                                                    style=TextStyle::CAPTION
                                                    color=TextColor::Danger
                                                >
                                                    {refusal}
                                                </Text>
                                            </Show>
                                        </List>
                                    </Frame>
                                </Scroll>
                            </Frame>
                        </DockTab>
                    </DockPane>
                </Show>
            </DockSplit>
        </Docking>
    }
}

#[component]
fn ImageArea(
    editor: Editor,
    image: Memo<Option<Image>>,
    thumbhash: Memo<Option<Thumbhash>>,
    error: Memo<Option<String>>,
) -> NodeId {
    view! {
        <List align=Align::Stretch spacing=Space::None>
            <ListItem grow=1.0>
                <Artwork editor={editor} image={image} thumbhash={thumbhash} />
            </ListItem>
            <Show value={error}>|reason: ReadSignal<String>|
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
    let placed = create_signal(Rect::ZERO);
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
    let built = view! {
        <Canvas view={editor.canvas()}>
            <CanvasItem
                x={move || shape.get().left()}
                y={move || shape.get().top()}
                width={move || shape.get().width()}
                height={move || shape.get().height()}
                @test_id={"image.picture"}
            >
                <Picture image={image} thumbhash={thumbhash} fit=ImageFit::Fill />
            </CanvasItem>
        </Canvas>
    };
    let placed_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = placed_measured.get() {
            placed.set(measured.rect);
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
        <Loading fallback>|| {
            let block = editor.block_content::<ImageContent>();
            let shown = watch(editor, block);
            let image = create_memo(move || shown.get().image);
            view! {
                <Picture image={image} thumbhash={placeholder} fit=ImageFit::Contain />
            }
        }</Loading>
    }
}
