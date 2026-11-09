use beui::Overflow;

use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{
    DockBar, DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize,
};
use be_sdk_beui::beui::icons::{
    ICON_ARROW_LEFT, ICON_ARROW_RIGHT, ICON_COPY_PLUS, ICON_ELLIPSIS, ICON_MAXIMIZE,
    ICON_STICKY_NOTE_PLUS, ICON_UNLINK, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Color32, CursorIcon, Key, KeyPress, NodeId, Vec2};
use be_sdk_beui::{ChildBlock, ChildBlockHandle, ChildMode, ChildState, Editor};
use uuid::Uuid;

use super::slides::{Notes, Slides};
use beui::{Align, Direction};

const FILMSTRIP: &str = "filmstrip";
const STAGE: &str = "stage";
const TOOLBAR: &str = "toolbar";
const NOTES: &str = "notes";
const FILMSTRIP_WIDTH: f32 = 240.0;
const NOTES_HEIGHT: f32 = 160.0;
const THUMBNAIL_WIDTH: Size = Size::Custom(176.0);
const PLAYBACK_HEIGHT: Size = Size::Xl2;
const SLIDE_HEIGHT: f32 = 540.0;
const DEFAULT_RATIO: f32 = 16.0 / 9.0;
const FILL: f32 = 0.0;
const FILMSTRIP_PADDING: Space = Space::Sm;
const NOTES_PADDING: Space = Space::Sm;
const TILE_INSET: Space = Space::Xs;
const TILE_SPACING: Space = Space::Md;
const PRESENTING: Color32 = Color32::BLACK;
const PLAYBACK_SHADE: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 180);

#[component]
pub fn PresentationView(editor: Editor) -> NodeId {
    let slides = Slides::new(editor);
    let presenting = editor.presenting();
    let chrome = editor.chrome_shown();
    let docked_stage = NodeRef::new();
    let bare_stage = NodeRef::new();

    let editing = create_memo(move || !presenting.get());
    let docked = create_memo(move || editing.get() && chrome.get());
    let bare = create_memo(move || !docked.get());
    create_effect(move || match docked.get() {
        true => editor.content(&docked_stage),
        false => editor.content(&bare_stage),
    });
    let background = create_memo(move || match presenting.get() {
        true => ColorRole::Custom(PRESENTING.into()),
        false => ColorRole::Background,
    });
    let keys = move |press: KeyPress| presenting.get_untracked() && navigate(slides, press);
    let read_only = editor.read_only();
    let duplicable = create_memo(move || {
        docked.get() && !read_only.get() && slides.target(slides.selected().get()).is_some()
    });
    let duplicate = Action::new("presentation.duplicate", "Duplicate slide", move || {
        if let Some(id) = slides.selected().get_untracked() {
            slides.duplicate(id);
        }
    })
    .glyph(ICON_COPY_PLUS)
    .shortcut(Chord::ctrl(Key::D))
    .enabled(duplicable)
    .register();

    view! {
        <Interactive focusable=true focused={presenting} on_key={keys}>
            <Frame fill={background}>
                <Layers>
                    <Show condition={docked}>
                        <DeckDock
                            editor={editor}
                            slides={slides}
                            duplicate={duplicate.clone()}
                            stage={docked_stage}
                            selected={slides.selected()}
                        />
                    </Show>
                    <Show condition={bare}>
                        <List spacing=Space::None>
                            <ListItem grow=1.0>
                                <Stage
                                    @node_ref={&bare_stage}
                                    editor={editor}
                                    slides={slides}
                                    selected={slides.selected()}
                                    mode=ChildMode::Live
                                    own_frame={editing}
                                    fit={presenting}
                                    report_size=true
                                />
                            </ListItem>
                            <Show condition={presenting}>
                                <Playback editor={editor} slides={slides} />
                            </Show>
                        </List>
                    </Show>
                </Layers>
            </Frame>
        </Interactive>
    }
}

#[component]
fn DeckDock(
    editor: Editor,
    slides: Slides,
    duplicate: Action,
    stage: NodeRef,
    selected: ReadSignal<Option<Uuid>>,
) -> NodeId {
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockSplit id="deck" size={SplitSize::First(FILMSTRIP_WIDTH)}>
                <DockPane id="filmstrip" locked=true>
                    <DockTab id=FILMSTRIP title="Slides">
                        <Filmstrip editor={editor} slides={slides} duplicate={duplicate.clone()} />
                    </DockTab>
                </DockPane>
                <DockSplit
                    id="slide"
                    direction=Direction::Vertical
                    size={SplitSize::Second(NOTES_HEIGHT)}
                >
                    <DockPane id="stage" locked=true bare=true>
                        <DockTab id=STAGE title="Slide">
                            <Stage
                                @node_ref={&stage}
                                editor={editor}
                                slides={slides}
                                selected={selected}
                                mode=ChildMode::Live
                                own_frame=true
                                fit=false
                                report_size=false
                            />
                        </DockTab>
                    </DockPane>
                    <DockPane id="notes" locked=true>
                        <DockTab id=NOTES title="Notes">
                            <SpeakerNotes editor={editor} slides={slides} />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </DockSplit>
            <DockBar
                id=TOOLBAR
                parent=STAGE
                placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
            >
                <Toolbar editor={editor} slides={slides} />
            </DockBar>
        </Docking>
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
        slides.add(index, add_button);
    };
    let present = move || editor.present(true);
    view! {
        <DockBarSurface spacing=Space::Md @test_id={"presentation.toolbar"}>
            <Frame @test_id={"presentation.toolbar.grip"}>
                <DockBarGrip />
            </Frame>
            <Text style=TextStyle::CAPTION @test_id={"presentation.position"}>{position}</Text>
            <Button
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
        </DockBarSurface>
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
                            fill={ColorRole::Custom(PLAYBACK_SHADE.into())}
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
                                    color={TextColor::Custom(Color32::WHITE.into())}
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
fn SpeakerNotes(editor: Editor, slides: Slides) -> NodeId {
    let read_only = editor.read_only();
    let selected = slides.selected();
    let notes = create_memo(move || {
        selected
            .get()
            .and_then(|id| slides.slide(id))
            .map_or(Notes::Loading, |slide| slide.notes)
    });
    let target = create_memo(move || match notes.get() {
        Notes::Block(target) => Some(target),
        Notes::None | Notes::Loading | Notes::Missing => None,
    });
    let missing = create_memo(move || notes.get() == Notes::None);
    let status = create_memo(move || match notes.get() {
        Notes::Loading => Some("Loading the notes…"),
        Notes::Missing => Some("This slide's notes could not be found."),
        Notes::None | Notes::Block(_) => None,
    });
    let waiting = create_memo(move || status.get().is_some());
    let message = create_memo(move || status.get().unwrap_or_default().to_owned());
    let written = create_memo(move || target.get().is_some());
    let focus = slides.focus_notes();
    let add = move || {
        if let Some(id) = selected.get_untracked() {
            slides.add_notes(id);
        }
    };
    let placed = move |state: ChildState| {
        let wanted = focus.get_untracked();
        if let Some(child) = state.child
            && wanted.is_some()
            && wanted == target.get_untracked().map(|target| target.id)
        {
            focus.set(None);
            editor.host().focus_child(child);
        }
    };
    view! {
        <Frame fill=ColorRole::Surface>
            <Layers>
                <List spacing=Space::None>
                    <Show condition={missing}>
                        <Frame padding=NOTES_PADDING>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <Text style=TextStyle::CAPTION>
                                    "This slide has no speaker notes."
                                </Text>
                                <Button
                                    glyph=ICON_STICKY_NOTE_PLUS
                                    label="Add notes"
                                    disabled={read_only}
                                    on_click={add}
                                    @test_id={"presentation.notes.add"}
                                />
                            </List>
                        </Frame>
                    </Show>
                    <Show condition={waiting}>
                        <Frame padding=NOTES_PADDING>
                            <Text style=TextStyle::CAPTION @test_id={"presentation.notes.status"}>
                                {message}
                            </Text>
                        </Frame>
                    </Show>
                </List>
                <Show condition={written}>
                    <ChildBlock
                        editor={editor}
                        block={target}
                        mode=ChildMode::Live
                        own_frame=true
                        on_state={placed}
                        @test_id={"presentation.notes"}
                    >|handle: ChildBlockHandle|
                        <SlideStatus state={handle.state} />
                    </ChildBlock>
                </Show>
            </Layers>
        </Frame>
    }
}

#[component]
fn Filmstrip(editor: Editor, slides: Slides, duplicate: Action) -> NodeId {
    let keys = slides.keys();
    let read_only = editor.read_only();
    let selection = create_selector(move || slides.selected().get());
    let empty = create_memo(move || slides.count() == 0);
    let moved = move |moved: ReorderMove<Uuid, Uuid>| slides.move_to(moved.payload, moved.index);
    let text = move |id: Uuid| {
        untrack(|| slides.slide(id))
            .map(|slide| slide.name)
            .unwrap_or_default()
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
                fill=ColorRole::Control
                border={Border::FOCUS}
                radius=Radius::Md
                padding=Space::Md
                @test_id="presentation.slide-ghost"
            >
                <Text style=TextStyle::CAPTION>{label}</Text>
            </Frame>
        }
    };

    view! {
        <Frame fill=ColorRole::Surface>
            <Container>|area: ReadSignal<Vec2>| {
                let direction = create_memo(move || {
                    let area = area.get();
                    match area.x > area.y {
                        true => Direction::Horizontal,
                        false => Direction::Vertical,
                    }
                });
                view! {
                    <List spacing=Space::None>
                        <Show condition={empty}>
                            <Frame padding=FILMSTRIP_PADDING>
                                <Text style=TextStyle::CAPTION>
                                    "Add a slide to start this deck."
                                </Text>
                            </Frame>
                        </Show>
                        <ListItem grow=1.0>
                            <Scroll direction={direction} @test_id={"presentation.filmstrip"}>
                                <Reorder
                                    keys={keys}
                                    direction={direction}
                                    payload={Func::new(Some)}
                                    disabled={read_only}
                                    text={text}
                                    preview={ghost}
                                    on_move={moved}
                                >|reorder: ReorderHandle<Uuid, Uuid>|
                                    <Frame padding=FILMSTRIP_PADDING>
                                        <List
                                            direction={direction}
                                            align=Align::Start
                                            spacing=TILE_SPACING
                                        >
                                            <ForEach keys={reorder.order}>|id: Uuid|
                                                <SlideTile
                                                    editor={editor}
                                                    slides={slides}
                                                    duplicate={duplicate.clone()}
                                                    reorder={reorder}
                                                    selected={selection.memo(Some(id))}
                                                    id={id}
                                                />
                                            </ForEach>
                                        </List>
                                    </Frame>
                                </Reorder>
                            </Scroll>
                        </ListItem>
                    </List>
                }
            }</Container>
        </Frame>
    }
}

#[component]
fn SlideTile(
    editor: Editor,
    slides: Slides,
    duplicate: Action,
    reorder: ReorderHandle<Uuid, Uuid>,
    selected: Prop<bool>,
    id: Uuid,
) -> NodeId {
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
    let remove = move || slides.remove(id);
    let menu = create_signal(false);
    create_effect(move || {
        if menu.get() {
            untrack(|| slides.select(Some(id)));
        }
    });
    let tile = NodeRef::new();
    let area = be_sdk_beui::beui::unstyled::container_size();
    create_effect(move || {
        if let Some(area) = area {
            area.get();
        }
        if selected.get()
            && let Some(node) = tile.try_get()
        {
            node.reveal();
        }
    });

    view! {
        <ReorderItem
            @node_ref=&tile
            reorder
            item=id
            margin={Padding::all(TILE_INSET)}
            cursor=CursorIcon::PointingHand
            on_click={select}
        >|_: ReorderItemHandle|
            <Frame
                fill=ColorRole::Control
                border={create_memo(move || Some(outline.get()))}
                radius=Radius::Md
                padding=Space::Sm
            >
                <List spacing=Space::Sm>
                    <Frame
                        width=THUMBNAIL_WIDTH
                        aspect_ratio={ratio}
                        @test_id={format!("presentation.slide.{id}")}
                    >
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
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                        <Text style=TextStyle::CAPTION>{number}</Text>
                        <ListItem grow=1.0>
                            <Text overflow=Overflow::Ellipsis style=TextStyle::CAPTION>{name}</Text>
                        </ListItem>
                        <MenuButton
                            icon_only=true
                            variant=ButtonVariant::Ghost
                            label="More"
                            glyph=ICON_ELLIPSIS
                            open={menu}
                            capture_presses=true
                            @test_id={format!("presentation.slide.{id}.more")}
                            items={view! {
                                <MenuItem
                                    action={duplicate}
                                    row_test_id={format!("presentation.slide.{id}.duplicate")}
                                />
                                <MenuItem
                                    label="Detach slide"
                                    glyph=ICON_UNLINK
                                    disabled={read_only}
                                    row_test_id={format!("presentation.slide.{id}.remove")}
                                    on_click={remove}
                                />
                            }}
                        />
                    </List>
                </List>
            </Frame>
        </ReorderItem>
    }
}
