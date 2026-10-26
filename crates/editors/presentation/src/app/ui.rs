use beui::Overflow;
use std::cell::Cell;
use std::rc::Rc;

use block_editor_beui::beui::icons::{
    ICON_ARROW_LEFT, ICON_ARROW_RIGHT, ICON_MAXIMIZE, ICON_TRASH, ICON_X,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, CursorIcon, Key, KeyPress, NodeId, Vec2};
use block_editor_beui::{ChildBlock, ChildBlockHandle, ChildMode, ChildState, Editor, node_anchor};
use uuid::Uuid;

use super::slides::Slides;

const FILMSTRIP_WIDTH: Size = Size::Custom(210.0);
const THUMBNAIL_WIDTH: Size = Size::Custom(176.0);
const PLAYBACK_HEIGHT: Size = Size::Xl2;
const SLIDE_HEIGHT: f32 = 540.0;
const DEFAULT_RATIO: f32 = 16.0 / 9.0;
const FILL: f32 = 0.0;
const PANEL_PADDING: Space = Space::Md;
const FILMSTRIP_PADDING: Space = Space::Sm;
const TILE_INSET: Space = Space::Xs;
const TILE_SPACING: Space = Space::Md;
const PRESENTING: Color32 = Color32::BLACK;
const PLAYBACK_SHADE: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 180);

#[component]
pub fn PresentationView(editor: Editor) -> NodeId {
    let slides = Slides::new(editor);
    let presenting = editor.presenting();
    let stage = NodeRef::new();
    editor.content(&stage);

    let editing = create_memo(move || !presenting.get());
    let background = create_memo(move || match presenting.get() {
        true => ColorRole::Custom(PRESENTING),
        false => ColorRole::Background,
    });
    let keys = move |press: KeyPress| presenting.get_untracked() && navigate(slides, press);

    view! {
        <Interactive focusable=true focused={presenting} on_key={keys}>
            <Frame fill={background}>
                <List direction=Direction::Horizontal spacing=Space::None>
                    <ShowKeepAlive condition={editing}>
                        <Filmstrip editor={editor} slides={slides} />
                        <Separator direction=Direction::Vertical />
                    </ShowKeepAlive>
                    <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                        <ShowKeepAlive condition={editing}>
                            <Toolbar editor={editor} slides={slides} />
                            <Separator />
                        </ShowKeepAlive>
                        <Stage
                            @sizing=ItemSize::Percent(100.0)
                            @node_ref={&stage}
                            editor={editor}
                            slides={slides}
                            selected={slides.selected()}
                            mode=ChildMode::Live
                            own_frame={editing}
                            fit={presenting}
                            report_size=true
                        />
                        <Show condition={presenting}>
                            <Playback editor={editor} slides={slides} />
                        </Show>
                    </List>
                </List>
            </Frame>
        </Interactive>
    }
}

#[component]
pub fn PresentationPreview(editor: Editor) -> NodeId {
    let slides = Slides::new(editor);
    let first = create_memo(move || slides.keys().with(|keys| keys.first().copied()));
    view! {
        <Stage
            editor={editor}
            slides={slides}
            selected={first}
            mode=ChildMode::Preview
            own_frame=false
            fit=true
            report_size=false
        />
    }
}

fn navigate(slides: Slides, press: KeyPress) -> bool {
    if !press.pressed {
        return false;
    }
    match press.key {
        Key::ArrowLeft | Key::PageUp => slides.step(-1),
        Key::ArrowRight | Key::PageDown | Key::Space => slides.step(1),
        Key::Home => slides.go_to(0),
        Key::End => slides.go_to(usize::MAX),
        _ => return false,
    }
    true
}

#[component]
fn Stage(
    editor: Editor,
    slides: Slides,
    selected: Prop<Option<Uuid>>,
    mode: Prop<ChildMode>,
    own_frame: Prop<bool>,
    fit: Prop<bool>,
    report_size: bool,
) -> NodeId {
    let target = create_memo(move || slides.target(selected.get()));
    let ratio = create_signal(DEFAULT_RATIO);
    let report = move |state: ChildState| {
        let aspect = state.aspect_ratio.unwrap_or(DEFAULT_RATIO);
        ratio.set(aspect);
        if report_size {
            editor.set_intrinsic_size(Some(Vec2::new(SLIDE_HEIGHT * aspect, SLIDE_HEIGHT)));
        }
    };
    let fitted = create_memo(move || match fit.get() {
        true => ratio.get(),
        false => FILL,
    });

    view! {
        <Frame aspect_ratio={fitted}>
            <ChildBlock
                editor={editor}
                block={target}
                mode={mode}
                own_frame={own_frame}
                on_state={report}
                @test_id={"presentation.stage"}
            >|handle: ChildBlockHandle|
                <SlideStatus state={handle.state} />
            </ChildBlock>
        </Frame>
    }
}

#[component]
fn SlideStatus(state: ReadSignal<ChildState>) -> NodeId {
    let hidden = create_memo(move || state.with(|state| !state.available || state.error.is_some()));
    let message = create_memo(move || {
        state.with(|state| match &state.error {
            Some(error) => error.clone(),
            None if state.placed => "Loading this slide…".to_owned(),
            None => "This slide's block could not be found.".to_owned(),
        })
    });
    view! {
        <Layers>
            <Show condition={hidden}>
                <Text size=TextSize::Sm color=TextColor::Muted>{message}</Text>
            </Show>
        </Layers>
    }
}

#[component]
fn Toolbar(editor: Editor, slides: Slides) -> NodeId {
    let read_only = editor.read_only();
    let empty = create_memo(move || slides.count() == 0);
    let position = create_memo(move || {
        let count = slides.count();
        match slides.selected().get().and_then(|id| slides.index_of(id)) {
            Some(index) => format!("Slide {} of {count}", index + 1),
            None => "No slides".to_owned(),
        }
    });
    let add_button = NodeRef::new();
    let add = move || {
        let index = slides.count();
        slides.add(index, node_anchor(&add_button));
    };
    let present = move || editor.present(true);
    view! {
        <Frame fill=ColorRole::Surface padding=PANEL_PADDING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Text style=TextStyle::CAPTION @test_id={"presentation.position"}>{position}</Text>
                <Button
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&add_button}
                    label="Add slide"
                    variant=ButtonVariant::Primary
                    disabled={read_only}
                    on_click={add}
                    @test_id={"presentation.add"}
                />
                <Button
                    icon_only=true
                    glyph=ICON_MAXIMIZE
                    label="Present"
                    disabled={empty}
                    on_click={present}
                    @test_id={"presentation.present"}
                />
            </List>
        </Frame>
    }
}

#[component]
fn Playback(editor: Editor, slides: Slides) -> NodeId {
    let over = create_signal(false);
    let position = create_memo(move || {
        let count = slides.count();
        let current = slides
            .selected()
            .get()
            .and_then(|id| slides.index_of(id))
            .map_or(0, |index| index + 1);
        format!("{current} / {count}")
    });
    let at_start =
        create_memo(move || slides.selected().get().and_then(|id| slides.index_of(id)) == Some(0));
    let at_end = create_memo(move || {
        let count = slides.count();
        slides.selected().get().and_then(|id| slides.index_of(id)) == count.checked_sub(1)
    });
    let previous = move || slides.step(-1);
    let next = move || slides.step(1);
    let stop = move || editor.present(false);

    view! {
        <Frame height=PLAYBACK_HEIGHT>
            <Interactive on_hover_change={over} @test_id={"presentation.playback"}>
                <Layers>
                    <Show condition={over}>
                        <Frame
                            fill={ColorRole::Custom(PLAYBACK_SHADE)}
                            padding={Padding::horizontal(Space::Lg)}
                        >
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <Button
                                    icon_only=true
                                    glyph=ICON_ARROW_LEFT
                                    label="Previous slide"
                                    disabled={at_start}
                                    on_click={previous}
                                    @test_id={"presentation.previous"}
                                />
                                <Text
                                    color={TextColor::Custom(Color32::WHITE)}
                                    @test_id={"presentation.playback.position"}
                                >
                                    {position}
                                </Text>
                                <Button
                                    icon_only=true
                                    glyph=ICON_ARROW_RIGHT
                                    label="Next slide"
                                    disabled={at_end}
                                    on_click={next}
                                    @test_id={"presentation.next"}
                                />
                                <Button
                                    icon_only=true
                                    glyph=ICON_X
                                    label="Stop presenting"
                                    on_click={stop}
                                    @test_id={"presentation.stop"}
                                />
                            </List>
                        </Frame>
                    </Show>
                </Layers>
            </Interactive>
        </Frame>
    }
}

#[component]
fn Filmstrip(editor: Editor, slides: Slides) -> NodeId {
    let keys = slides.keys();
    let selection = create_selector(move || slides.selected().get());
    let empty = create_memo(move || slides.count() == 0);
    let tiles = move |id: Uuid| {
        view! {
            <SlideTile
                editor={editor}
                slides={slides}
                selected={selection.memo(Some(id))}
                id={id}
            />
        }
    };

    view! {
        <Frame width=FILMSTRIP_WIDTH fill=ColorRole::Surface padding=FILMSTRIP_PADDING>
            <List spacing=TILE_SPACING>
                <Show condition={empty}>
                    <Text style=TextStyle::CAPTION>"Add a slide to start this deck."</Text>
                </Show>
                <Scroll @sizing=ItemSize::Percent(100.0) @test_id={"presentation.filmstrip"}>
                    <ForEach keys={keys} view={tiles} />
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn SlideTile(editor: Editor, slides: Slides, selected: Prop<bool>, id: Uuid) -> NodeId {
    let editable = editor.editable();
    let read_only = editor.read_only();
    let ratio = create_signal(DEFAULT_RATIO);
    let target = create_memo(move || slides.target(Some(id)));
    let name = create_memo(move || {
        slides
            .slide(id)
            .map_or_else(|| "Loading…".to_owned(), |slide| slide.name)
    });
    let number = create_memo(move || {
        slides
            .index_of(id)
            .map_or_else(String::new, |index| (index + 1).to_string())
    });
    let selected = create_memo(move || selected.get());
    let outline = create_memo(move || match selected.get() {
        true => Border::new(ColorRole::Accent).width(Stroke::Focus),
        false => Border::DEFAULT,
    });

    let select = move || slides.select(Some(id));
    let payload = create_memo(move || editable.get().then_some(id));
    let entered = Rc::new(Cell::new(false));
    let over = move |over: Option<(Uuid, DragPoint)>| {
        let Some((held, _)) = over else {
            entered.set(false);
            return;
        };
        if entered.replace(true) || held == id {
            return;
        }
        if let Some(index) = slides.index_of(id) {
            slides.move_to(held, index);
        }
    };
    let ghost = move |held: Uuid| {
        let label = create_memo(move || {
            let number = slides.index_of(held).map_or(0, |index| index + 1);
            let name = slides
                .slide(held)
                .map(|slide| slide.name)
                .unwrap_or_default();
            format!("{number}  {name}")
        });
        view! {
            <Frame
                width=THUMBNAIL_WIDTH
                fill=ColorRole::SurfaceRaised
                border={Border::FOCUS}
                radius=Radius::Md
                padding=Space::Md
                @test_id="presentation.slide-ghost"
            >
                <Text style=TextStyle::CAPTION>{label}</Text>
            </Frame>
        }
    };
    let remove = move || slides.remove(id);

    view! {
        <Frame padding=TILE_INSET>
            <Frame
                fill=ColorRole::SurfaceRaised
                border={create_memo(move || Some(outline.get()))}
                radius=Radius::Md
                padding=Space::Sm
            >
                <List spacing=Space::Sm>
                    <DropTarget on_over={over}>|_: DropHandle|
                        <Draggable
                            payload={payload}
                            cursor=CursorIcon::PointingHand
                            preview={ghost}
                            on_click={select}
                            @test_id={format!("presentation.slide.{id}")}
                        >|_: DragHandle|
                            <Frame width=THUMBNAIL_WIDTH aspect_ratio={ratio}>
                                <ChildBlock
                                    editor={editor}
                                    block={target}
                                    mode=ChildMode::Preview
                                    on_state={move |state: ChildState| {
                                        ratio.set(state.aspect_ratio.unwrap_or(DEFAULT_RATIO));
                                    }}
                                >|handle: ChildBlockHandle|
                                    <SlideStatus state={handle.state} />
                                </ChildBlock>
                            </Frame>
                        </Draggable>
                    </DropTarget>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                        <Text style=TextStyle::CAPTION>{number}</Text>
                        <Text
                            overflow=Overflow::Ellipsis
                            style=TextStyle::CAPTION
                            @sizing=ItemSize::Percent(100.0)
                        >
                            {name}
                        </Text>
                        <Button
                            icon_only=true
                            glyph=ICON_TRASH
                            label="Detach slide"
                            disabled={read_only}
                            on_click={remove}
                            @test_id={format!("presentation.slide.{id}.remove")}
                        />
                    </List>
                </List>
            </Frame>
        </Frame>
    }
}
