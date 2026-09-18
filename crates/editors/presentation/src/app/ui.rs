use std::rc::Rc;

use block_editor_plugin::beui::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_CLOSE, ICON_DELETE, ICON_FULLSCREEN,
};
use block_editor_plugin::beui::reactive::{
    CenteredRow, ClickCatcher, Column, Focusable, ForEach, Frame, ItemSize, NodeRef, Prop,
    ReadSignal, Row, Scroll, Show, Text, WriteSignal, clone, component, create_memo,
    create_selector, create_signal, view,
};
use block_editor_plugin::beui::styled::{
    Button, ButtonVariant, Caption, IconButton, theme, use_theme,
};
use block_editor_plugin::beui::{Color32, CursorIcon, Key, KeyPress, NodeId, Vec2};
use block_editor_plugin::{ChildBlock, ChildBlockHandle, ChildMode, ChildState, Editor};
use uuid::Uuid;

use super::slides::Slides;

const FILMSTRIP_WIDTH: f32 = 210.0;
const THUMBNAIL_WIDTH: f32 = 176.0;
const PLAYBACK_HEIGHT: f32 = 48.0;
const SLIDE_HEIGHT: f32 = 540.0;
const DEFAULT_RATIO: f32 = 16.0 / 9.0;
const FILL: f32 = 0.0;
const PANEL_PADDING: f32 = 10.0;
const FILMSTRIP_PADDING: f32 = 6.0;
const TILE_INSET: f32 = 4.0;
const TILE_SPACING: f32 = 8.0;

#[derive(Clone)]
struct Drag {
    held: ReadSignal<Option<Uuid>>,
    set_held: WriteSignal<Option<Uuid>>,
    over: ReadSignal<Option<Uuid>>,
    set_over: WriteSignal<Option<Uuid>>,
}

impl Drag {
    fn new() -> Self {
        let (held, set_held) = create_signal(None::<Uuid>);
        let (over, set_over) = create_signal(None::<Uuid>);
        Self {
            held,
            set_held,
            over,
            set_over,
        }
    }
}

#[component]
pub fn PresentationView(editor: Editor) -> NodeId {
    let slides = Slides::new(&editor);
    let presenting = editor.presenting();
    let stage = NodeRef::new();
    editor.content(&stage);

    let theme = use_theme();
    let editing = create_memo(clone!(presenting -> move || !presenting.get()));
    let mode = create_memo(clone!(presenting -> move || match presenting.get() {
        true => ChildMode::Preview,
        false => ChildMode::Live,
    }));
    let background = create_memo(clone!(theme presenting -> move || match presenting.get() {
        true => Color32::BLACK,
        false => theme.background.get(),
    }));
    let keys = clone!(slides presenting -> move |press: KeyPress| {
        presenting.get_untracked() && navigate(&slides, press)
    });

    view! {
        <Focusable focused={presenting.clone()} on_key={keys}>
            <Frame color={background}>
                <Row spacing=0.0>
                    <Filmstrip
                        @sizing=ItemSize::Fixed(FILMSTRIP_WIDTH)
                        editor={editor.clone()}
                        slides={Rc::clone(&slides)}
                        shown={editing.clone()}
                    />
                    <Frame
                        visible={editing.clone()}
                        width=theme::BORDER_WIDTH
                        color={theme.border.clone()}
                    />
                    <Column @sizing=ItemSize::Percent(100.0) spacing=0.0>
                        <Toolbar
                            editor={editor.clone()}
                            slides={Rc::clone(&slides)}
                            shown={editing.clone()}
                        />
                        <Frame
                            visible={editing.clone()}
                            height=theme::BORDER_WIDTH
                            color={theme.border.clone()}
                        />
                        <Stage
                            @sizing=ItemSize::Percent(100.0)
                            @node_ref={&stage}
                            editor={editor.clone()}
                            slides={Rc::clone(&slides)}
                            selected={slides.selected()}
                            mode={mode}
                            own_frame={editing}
                            fit={presenting.clone()}
                            report_size=true
                        />
                        <Playback editor={editor} slides={slides} shown={presenting} />
                    </Column>
                </Row>
            </Frame>
        </Focusable>
    }
}

#[component]
pub fn PresentationPreview(editor: Editor) -> NodeId {
    let slides = Slides::new(&editor);
    let first =
        create_memo(clone!(slides -> move || slides.keys().with(|keys| keys.first().copied())));
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

fn navigate(slides: &Rc<Slides>, press: KeyPress) -> bool {
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
    slides: Rc<Slides>,
    selected: Prop<Option<Uuid>>,
    mode: Prop<ChildMode>,
    own_frame: Prop<bool>,
    fit: Prop<bool>,
    report_size: bool,
) -> NodeId {
    let target = create_memo(clone!(slides -> move || slides.target(selected.get())));
    let (ratio, set_ratio) = create_signal(DEFAULT_RATIO);
    let sized = editor.clone();
    let report = move |state: ChildState| {
        let ratio = state.aspect_ratio.unwrap_or(DEFAULT_RATIO);
        set_ratio.set(ratio);
        if report_size {
            sized.set_intrinsic_size(Some(Vec2::new(SLIDE_HEIGHT * ratio, SLIDE_HEIGHT)));
        }
    };
    let fitted = create_memo(clone!(ratio -> move || match fit.get() {
        true => ratio.get(),
        false => FILL,
    }));

    view! {
        <Frame aspect_ratio={fitted}>
            <ChildBlock
                editor={editor}
                block={target}
                mode={mode}
                own_frame={own_frame}
                on_state={report}
                @test_id={"presentation.stage"}
            >
                {move |handle: ChildBlockHandle| view! {
                    <SlideStatus state={handle.state} />
                }}
            </ChildBlock>
        </Frame>
    }
}

#[component]
fn SlideStatus(state: ReadSignal<ChildState>) -> NodeId {
    let theme = use_theme();
    let hidden = create_memo(clone!(state -> move || {
        state.with(|state| !state.available || state.error.is_some())
    }));
    let message = create_memo(
        clone!(state -> move || state.with(|state| match &state.error {
            Some(error) => error.clone(),
            None if state.placed => "Loading this slide…".to_owned(),
            None => "This slide's block could not be found.".to_owned(),
        })),
    );
    view! {
        <Frame visible={hidden}>
            <Text
                string={message}
                font_size=theme::FONT_SMALL
                color={theme.text_muted.clone()}
                wrap=true
            />
        </Frame>
    }
}

#[component]
fn Toolbar(editor: Editor, slides: Rc<Slides>, shown: Prop<bool>) -> NodeId {
    let theme = use_theme();
    let editable = editor.editable();
    let empty = create_memo(clone!(slides -> move || slides.count() == 0));
    let position = create_memo(clone!(slides -> move || {
        let count = slides.count();
        match slides.selected().get().and_then(|id| slides.index_of(id)) {
            Some(index) => format!("Slide {} of {count}", index + 1),
            None => "No slides".to_owned(),
        }
    }));
    let add = clone!(slides -> move || {
        let index = slides.count();
        slides.add(index);
    });
    let present = clone!(editor -> move || editor.present(true));
    view! {
        <Frame
            visible={shown}
            color={theme.surface.clone()}
            padding_horizontal=PANEL_PADDING
            padding_vertical=PANEL_PADDING
        >
            <CenteredRow spacing=8.0>
                <Caption content={position} @test_id={"presentation.position"} />
                <Button
                    @sizing=ItemSize::Percent(100.0)
                    label="Add slide"
                    variant=ButtonVariant::Primary
                    disabled={!editable}
                    on_click={add}
                    @test_id={"presentation.add"}
                />
                <IconButton
                    glyph={ICON_FULLSCREEN.to_owned()}
                    label="Present"
                    disabled={empty}
                    on_click={present}
                    @test_id={"presentation.present"}
                />
            </CenteredRow>
        </Frame>
    }
}

#[component]
fn Playback(editor: Editor, slides: Rc<Slides>, shown: Prop<bool>) -> NodeId {
    let (over, set_over) = create_signal(false);
    let position = create_memo(clone!(slides -> move || {
        let count = slides.count();
        let current = slides
            .selected()
            .get()
            .and_then(|id| slides.index_of(id))
            .map_or(0, |index| index + 1);
        format!("{current} / {count}")
    }));
    let at_start = create_memo(
        clone!(slides -> move || slides.selected().get().and_then(|id| slides.index_of(id)) == Some(0)),
    );
    let at_end = create_memo(clone!(slides -> move || {
        let count = slides.count();
        slides.selected().get().and_then(|id| slides.index_of(id)) == count.checked_sub(1)
    }));
    let previous = clone!(slides -> move || slides.step(-1));
    let next = clone!(slides -> move || slides.step(1));
    let stop = move || editor.present(false);

    view! {
        <Frame visible={shown} height=PLAYBACK_HEIGHT>
            <ClickCatcher
                on_hover_change={move |hovered| set_over.set(hovered)}
                @test_id={"presentation.playback"}
            >
                <Frame
                    visible={over}
                    color=Color32::from_rgba_unmultiplied(0, 0, 0, 180)
                    padding_horizontal=12.0
                >
                    <CenteredRow spacing=10.0>
                        <IconButton
                            glyph={ICON_ARROW_BACK.to_owned()}
                            label="Previous slide"
                            disabled={at_start}
                            on_click={previous}
                            @test_id={"presentation.previous"}
                        />
                        <Text
                            string={position}
                            color=Color32::WHITE
                            @test_id={"presentation.playback.position"}
                        />
                        <IconButton
                            glyph={ICON_ARROW_FORWARD.to_owned()}
                            label="Next slide"
                            disabled={at_end}
                            on_click={next}
                            @test_id={"presentation.next"}
                        />
                        <IconButton
                            glyph={ICON_CLOSE.to_owned()}
                            label="Stop presenting"
                            on_click={stop}
                            @test_id={"presentation.stop"}
                        />
                    </CenteredRow>
                </Frame>
            </ClickCatcher>
        </Frame>
    }
}

#[component]
fn Filmstrip(editor: Editor, slides: Rc<Slides>, shown: Prop<bool>) -> NodeId {
    let theme = use_theme();
    let drag = Drag::new();
    let keys = slides.keys();
    let selection = create_selector(clone!(slides -> move || slides.selected().get()));
    let empty = create_memo(clone!(slides -> move || slides.count() == 0));
    let tiles = clone!(editor slides drag -> move |id: Uuid| {
        view! {
            <SlideTile
                editor={editor.clone()}
                slides={Rc::clone(&slides)}
                drag={drag.clone()}
                selected={selection.memo(Some(id))}
                id={id}
            />
        }
    });

    view! {
        <Frame
            visible={shown}
            color={theme.surface.clone()}
            padding_horizontal=FILMSTRIP_PADDING
            padding_vertical=FILMSTRIP_PADDING
        >
            <Column spacing=TILE_SPACING>
                <Show condition={empty}>
                    <Caption content="Add a slide to start this deck." />
                </Show>
                <Scroll
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={"presentation.filmstrip"}
                    focus_color={theme.accent.clone()}
                >
                    <Column spacing=0.0>
                        <ForEach keys={keys} view={tiles} />
                    </Column>
                </Scroll>
            </Column>
        </Frame>
    }
}

#[component]
fn SlideTile(
    editor: Editor,
    slides: Rc<Slides>,
    drag: Drag,
    selected: Prop<bool>,
    id: Uuid,
) -> NodeId {
    let theme = use_theme();
    let editable = editor.editable();
    let (ratio, set_ratio) = create_signal(DEFAULT_RATIO);
    let target = create_memo(clone!(slides -> move || slides.target(Some(id))));
    let name = create_memo(clone!(slides -> move || {
        slides
            .slide(id)
            .map_or_else(|| "Loading…".to_owned(), |slide| slide.name)
    }));
    let number = create_memo(clone!(slides -> move || {
        slides
            .index_of(id)
            .map_or_else(String::new, |index| (index + 1).to_string())
    }));
    let selected = create_memo(move || selected.get());
    let outline = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let width = create_memo(clone!(selected -> move || match selected.get() {
        true => 2.0_f32,
        false => 1.0_f32,
    }));

    let select = clone!(slides -> move || slides.select(Some(id)));
    let hold = clone!(drag -> move |_| {
        if editable {
            drag.set_held.set(Some(id));
        }
    });
    let reorder = clone!(drag slides -> move |_| {
        if drag.held.get_untracked() != Some(id) {
            return;
        }
        let Some(over) = drag.over.get_untracked() else {
            return;
        };
        if over == id {
            return;
        }
        let Some(index) = slides.index_of(over) else {
            return;
        };
        slides.move_to(id, index);
    });
    let enter = clone!(drag -> move |hovered: bool| match hovered {
        true => drag.set_over.set(Some(id)),
        false if drag.over.get_untracked() == Some(id) => drag.set_over.set(None),
        false => {}
    });
    let release = clone!(drag -> move |active: bool| {
        if !active {
            drag.set_held.set(None);
        }
    });
    let remove = clone!(slides -> move || slides.remove(id));

    view! {
        <Frame padding_horizontal=TILE_INSET padding_vertical=TILE_INSET>
            <Frame
                color={theme.surface_raised.clone()}
                outline={outline}
                outline_width={width}
                outline_visible=true
                radius=theme::RADIUS
                padding_horizontal=6.0
                padding_vertical=6.0
            >
                <Column spacing=6.0>
                    <ClickCatcher
                        cursor=CursorIcon::PointingHand
                        on_click={select}
                        on_press={hold}
                        on_drag={reorder}
                        on_active_change={release}
                        on_hover_change={enter}
                        @test_id={format!("presentation.slide.{id}")}
                    >
                        <Frame width=THUMBNAIL_WIDTH aspect_ratio={ratio}>
                            <ChildBlock
                                editor={editor}
                                block={target}
                                mode=ChildMode::Preview
                                on_state={move |state: ChildState| {
                                    set_ratio.set(state.aspect_ratio.unwrap_or(DEFAULT_RATIO));
                                }}
                            >
                                {move |handle: ChildBlockHandle| view! {
                                    <SlideStatus state={handle.state} />
                                }}
                            </ChildBlock>
                        </Frame>
                    </ClickCatcher>
                    <CenteredRow spacing=6.0>
                        <Caption content={number} />
                        <Caption @sizing=ItemSize::Percent(100.0) content={name} />
                        <IconButton
                            glyph={ICON_DELETE.to_owned()}
                            label="Detach slide"
                            disabled={!editable}
                            on_click={remove}
                            @test_id={format!("presentation.slide.{id}.remove")}
                        />
                    </CenteredRow>
                </Column>
            </Frame>
        </Frame>
    }
}
