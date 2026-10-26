use block_editor_beui::Fit;
use block_editor_beui::beui::icons::{ICON_ARROW_LEFT, ICON_ARROW_RIGHT};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, ImageFit, NodeId, PointerPress, Pos2, Rect, Vec2};
use block_editor_beui::{Editor, FileChooser, Sidebar, Toolbar};

use super::pages::{Pages, Shown, Viewport};
use super::{filter, imported};
use pdf_types::PdfContent;

const PAGE_FILL: Color32 = Color32::from_rgb(255, 255, 255);

#[component]
pub fn PdfEditor(editor: Editor) -> NodeId {
    let block = editor.block_content::<PdfContent>();
    let pages = Pages::new();
    let chooser = FileChooser::new(filter(), imported);
    let shown = pages.shown();
    let performance = editor
        .host()
        .performance(format!("PDF ({})", editor.block_id()));

    chooser.on_chosen(move |chooser| {
        if let Some(pdf) = chooser.take() {
            editor.replace_content(editor.block_id(), &pdf);
            pages.go(0);
        }
    });
    let placed = editor.placed();
    let canvas = editor.canvas();
    let device = editor.pixels_per_point();
    let (waker, rendered) = editor.woken();
    create_effect(move || {
        rendered.get();
        let _measure = performance.measure("Editor pump");
        let size = pages.shown().get_untracked().size();
        pages.pump(
            &block,
            &performance,
            waker.clone(),
            viewport(placed.get(), canvas.get(), size, device.get()),
        );
    });

    create_effect(move || {
        let size = shown.get().size();
        editor.set_intrinsic_size(Some(size));
    });

    let page_label = create_memo(move || {
        let shown = shown.get();
        match shown.page_count {
            Some(count) => format!("Page {} of {count}", shown.page + 1),
            None => "Loading…".to_owned(),
        }
    });
    let first = create_memo(move || shown.get().page == 0);
    let last = create_memo(move || {
        let shown = shown.get();
        shown
            .page_count
            .is_some_and(|count| shown.page + 1 >= count)
    });
    let back = move || pages.go(pages.page().saturating_sub(1));
    let forward = move || pages.go(pages.page() + 1);
    let fitting = editor.keep_fitted(move || {
        let size = shown.get().size();
        (size.x > 0.0 && size.y > 0.0).then(|| Fit::Area(Rect::from_min_size(Pos2::ZERO, size)))
    });
    let fit = move || fitting.fit();

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(move || read_only.get() || busy.get());
    let failure = chooser.error();
    let refused = create_memo(move || failure.get().is_some());
    let refusal = create_memo(move || failure.get().unwrap_or_default());
    let choose = move || chooser.open();
    let source = block.project(|pdf| pdf.header().source_name.clone());
    let name = create_memo(move || source.get());

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List spacing=Space::None>
            <Toolbar shown={chrome}>
                <Text>"PDF"</Text>
                <Button
                    icon_only=true
                    glyph=ICON_ARROW_LEFT
                    label="Previous page"
                    disabled={first}
                    @test_id={"pdf.previous"}
                    on_click={back}
                />
                <Text style=TextStyle::CAPTION>{page_label}</Text>
                <Button
                    icon_only=true
                    glyph=ICON_ARROW_RIGHT
                    label="Next page"
                    disabled={last}
                    @test_id={"pdf.next"}
                    on_click={forward}
                />
                <Button
                    label="Fit view"
                    variant=ButtonVariant::Secondary
                    @test_id={"pdf.fit"}
                    on_click={fit}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </Toolbar>
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=Space::None>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                    <Interactive
                        on_press={move |_: PointerPress| editor.pan_drag()}
                        on_pan_drag={move |delta: Vec2| editor.pan(delta)}
                    >
                        <PageCanvas shown={shown} view={editor.canvas()} />
                    </Interactive>
                </Frame>
                <Sidebar shown={chrome}>
                    <Text style=TextStyle::HEADING>"PDF"</Text>
                    <Text style=TextStyle::CAPTION>{name}</Text>
                    <Button
                        label="Replace PDF…"
                        variant=ButtonVariant::Secondary
                        disabled={blocked}
                        @test_id={"pdf.replace"}
                        on_click={choose}
                    />
                    <Show condition={refused}>
                        <Text style=TextStyle::CAPTION color=TextColor::Danger>{refusal}</Text>
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </Sidebar>
            </List>
        </List>
    }
}

#[component]
fn PageCanvas(
    shown: Memo<Shown>,
    #[prop(default = None)] view: block_editor_beui::beui::reactive::Prop<Option<CanvasView>>,
) -> NodeId {
    let size = create_memo(move || shown.get().size());
    let width = create_memo(move || size.get().x);
    let height = create_memo(move || size.get().y);
    let keys =
        create_memo(move || (0..shown.with(|shown| shown.tiles.len())).collect::<Vec<usize>>());
    let failed = create_memo(move || shown.get().error.is_some());
    let reason = create_memo(move || shown.get().error.unwrap_or_default());
    view! {
        <List spacing=Space::None>
            <Canvas @sizing=ItemSize::Percent(100.0) view={view}>
                <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                    <Frame fill={ColorRole::Custom(PAGE_FILL)} />
                </CanvasItem>
                <ForEach keys={keys}>|index: usize| {
                    let tile = create_memo(move || {
                        shown.with(|shown| shown.tiles.get(index).cloned())
                    });
                    let x = create_memo(move || {
                        tile.get().map_or(0.0, |(rect, _)| rect.min.x)
                    });
                    let y = create_memo(move || {
                        tile.get().map_or(0.0, |(rect, _)| rect.min.y)
                    });
                    let width = create_memo(move || {
                        tile.get().map_or(0.0, |(rect, _)| rect.width())
                    });
                    let height = create_memo(move || {
                        tile.get().map_or(0.0, |(rect, _)| rect.height())
                    });
                    let image = create_memo(move || {
                        tile.get().map(|(_, image)| image)
                    });
                    let element_test_id = format!("pdf.tile.{index}");
                    view! {
                        <CanvasItem x={x} y={y} width={width} height={height}>
                            <Picture image={image} fit=ImageFit::Fill @test_id={element_test_id} />
                        </CanvasItem>
                    }
                }</ForEach>
            </Canvas>
            <Show condition={failed}>
                <Text style=TextStyle::CAPTION color=TextColor::Danger @test_id={"pdf.error"}>
                    {reason}
                </Text>
            </Show>
        </List>
    }
}

#[component]
pub fn PdfPreview(editor: Editor) -> NodeId {
    let block = editor.block_content::<PdfContent>();
    let pages = Pages::new();
    let shown = pages.shown();
    let performance = editor
        .host()
        .performance(format!("PDF preview ({})", editor.block_id()));
    let placed = editor.placed();
    let device = editor.pixels_per_point();
    let (waker, rendered) = editor.woken();
    create_effect(move || {
        rendered.get();
        let _measure = performance.measure("Preview pump");
        let size = pages.shown().get_untracked().size();
        pages.pump(
            &block,
            &performance,
            waker.clone(),
            viewport(placed.get(), None, size, device.get()),
        );
    });
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <Frame @node_ref={&content}>
            <PageCanvas shown={shown} />
        </Frame>
    }
}

fn viewport(
    content: Rect,
    view: Option<CanvasView>,
    page_size: Vec2,
    pixels_per_point: f32,
) -> Viewport {
    let page_rect = match view {
        Some(view) => Rect::from_min_max(
            view.to_screen(Pos2::ZERO),
            view.to_screen(Pos2::new(page_size.x, page_size.y)),
        ),
        None => fitted(content, page_size),
    };
    Viewport {
        page_rect,
        visible: page_rect.intersect(content),
        pixels_per_point,
    }
}

fn fitted(content: Rect, page_size: Vec2) -> Rect {
    if !content.is_positive() || page_size.x <= 0.0 || page_size.y <= 0.0 {
        return content;
    }
    let scale = (content.width() / page_size.x).min(content.height() / page_size.y);
    let size = Vec2::new(page_size.x * scale, page_size.y * scale);
    Rect::from_min_size(
        Pos2::new(
            content.left() + (content.width() - size.x) / 2.0,
            content.top() + (content.height() - size.y) / 2.0,
        ),
        size,
    )
}
