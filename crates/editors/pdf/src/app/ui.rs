use std::rc::Rc;

use block_editor_plugin::be_block::PdfContent;
use block_editor_plugin::beui::icons::{ICON_ARROW_BACK, ICON_ARROW_FORWARD};
use block_editor_plugin::beui::reactive::{
    Canvas, CanvasItem, CanvasView, ClickCatcher, Direction, ForEach, Frame, ItemSize, List, Memo,
    NodeRef, Picture, Show, Spacer, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Body, Button, ButtonVariant, Caption, Heading, use_theme};
use block_editor_plugin::beui::{Color32, ImageFit, NodeId, Pos2, Rect, Vec2};
use block_editor_plugin::{Editor, FileChooser, Sidebar, Toolbar};

use super::pages::{Pages, Shown, Viewport};
use super::{filter, imported};

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

    let pumping = Rc::clone(&pages);
    let polled = Rc::clone(&chooser);
    let replacing = editor.clone();
    let host = editor.host().clone();
    let frame = editor.clone();
    let canvas = editor.canvas();
    let device = editor.pixels_per_point();
    let block_for_pump = Rc::clone(&block);
    editor.each_frame(move || {
        let _measure = performance.measure("Editor frame");
        polled.poll(&host);
        if let Some(pdf) = polled.take() {
            replacing.replace_content(replacing.block_id(), &pdf);
            pumping.go(0);
        }
        let content = frame.content_rect();
        let size = pumping.shown().get_untracked().size();
        let view = canvas.get_untracked();
        pumping.pump(
            &frame,
            &block_for_pump,
            &performance,
            viewport(content, view, size, device.get_untracked()),
        );
    });

    let sized = editor.clone();
    let measured = shown.clone();
    block_editor_plugin::beui::reactive::create_effect(move || {
        let size = measured.get().size();
        sized.set_intrinsic_size(Some(size));
    });

    let page_label = create_memo(clone!(shown -> move || {
        let shown = shown.get();
        match shown.page_count {
            Some(count) => format!("Page {} of {count}", shown.page + 1),
            None => "Loading…".to_owned(),
        }
    }));
    let first = create_memo(clone!(shown -> move || shown.get().page == 0));
    let last = create_memo(clone!(shown -> move || {
        let shown = shown.get();
        shown.page_count.is_some_and(|count| shown.page + 1 >= count)
    }));
    let back = clone!(pages -> move || pages.go(pages.page().saturating_sub(1)));
    let forward = clone!(pages -> move || pages.go(pages.page() + 1));
    let fitting = editor.clone();
    let fit = move || fitting.fit();

    let read_only = editor.read_only();
    let busy = chooser.busy();
    let blocked = create_memo(clone!(read_only busy -> move || read_only.get() || busy.get()));
    let failure = chooser.error();
    let refused = create_memo(clone!(failure -> move || failure.get().is_some()));
    let refusal = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let opening = editor.host().clone();
    let choose = clone!(chooser -> move || chooser.open(&opening));
    let source = block.project(|pdf| pdf.header().source_name.clone());
    let name = create_memo(clone!(source -> move || source.get()));

    let panning = editor.clone();
    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    let danger = theme.danger.clone();
    view! {
        <List spacing=0.0>
            <Toolbar shown={chrome.clone()}>
                <Body content="PDF" />
                <block_editor_plugin::beui::styled::IconButton
                    glyph={ICON_ARROW_BACK.to_owned()}
                    label="Previous page"
                    disabled={first}
                    @test_id={"pdf.previous"}
                    on_click={back}
                />
                <Caption content={page_label} />
                <block_editor_plugin::beui::styled::IconButton
                    glyph={ICON_ARROW_FORWARD.to_owned()}
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
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content}>
                    <ClickCatcher on_pan_drag={move |delta: Vec2| panning.pan(delta)}>
                        <PageCanvas shown={shown.clone()} view={editor.canvas()} />
                    </ClickCatcher>
                </Frame>
                <Sidebar shown={chrome}>
                    <Heading content="PDF" />
                    <Caption content={name} />
                    <Button
                        label="Replace PDF…"
                        variant=ButtonVariant::Secondary
                        disabled={blocked}
                        @test_id={"pdf.replace"}
                        on_click={choose}
                    />
                    <Show condition={refused}>
                        <Caption content={refusal} color={danger} />
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
    #[prop(default = None)] view: block_editor_plugin::beui::reactive::Prop<Option<CanvasView>>,
) -> NodeId {
    let size = create_memo(clone!(shown -> move || shown.get().size()));
    let width = create_memo(clone!(size -> move || size.get().x));
    let height = create_memo(clone!(size -> move || size.get().y));
    let keys = create_memo(clone!(shown -> move || {
        (0..shown.with(|shown| shown.tiles.len())).collect::<Vec<usize>>()
    }));
    let failed = create_memo(clone!(shown -> move || shown.get().error.is_some()));
    let reason = create_memo(clone!(shown -> move || shown.get().error.unwrap_or_default()));
    let theme = use_theme();
    view! {
        <List spacing=0.0>
            <Canvas @sizing=ItemSize::Percent(100.0) view={view}>
                <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                    <Frame color=PAGE_FILL />
                </CanvasItem>
                <ForEach keys={keys}>
                    {move |index: usize| {
                        let tile = create_memo(clone!(shown -> move || {
                            shown.with(|shown| shown.tiles.get(index).cloned())
                        }));
                        let x = create_memo(clone!(tile -> move || {
                            tile.get().map_or(0.0, |(rect, _)| rect.min.x)
                        }));
                        let y = create_memo(clone!(tile -> move || {
                            tile.get().map_or(0.0, |(rect, _)| rect.min.y)
                        }));
                        let width = create_memo(clone!(tile -> move || {
                            tile.get().map_or(0.0, |(rect, _)| rect.width())
                        }));
                        let height = create_memo(clone!(tile -> move || {
                            tile.get().map_or(0.0, |(rect, _)| rect.height())
                        }));
                        let image = create_memo(clone!(tile -> move || {
                            tile.get().map(|(_, image)| image)
                        }));
                        view! {
                            <CanvasItem x={x} y={y} width={width} height={height}>
                                <Picture image={image} fit=ImageFit::Fill />
                            </CanvasItem>
                        }
                    }}
                </ForEach>
            </Canvas>
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
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
    let pumping = Rc::clone(&pages);
    let frame = editor.clone();
    let device = editor.pixels_per_point();
    let watched = Rc::clone(&block);
    editor.each_frame(move || {
        let _measure = performance.measure("Preview frame");
        let content = frame.content_rect();
        let size = pumping.shown().get_untracked().size();
        pumping.pump(
            &frame,
            &watched,
            &performance,
            viewport(content, None, size, device.get_untracked()),
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
