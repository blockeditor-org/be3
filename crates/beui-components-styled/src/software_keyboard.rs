use beui_components_unstyled as unstyled;
use beui_components_unstyled::software_keyboard::{
    EMOJI_GROUPS, Face, KeyCapHandle, KeyboardHandle, Latch, Layer, Role, Slot, Value, emojis_in,
    face,
};
use beui_core::context::Context;
use beui_core::document::Document;
use beui_core::geometry::Rect;
use beui_core::icons;
use beui_core::input::Key;
use beui_core::node::NodeId;
use beui_core::software_keyboard::KeyboardHost;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::frame::{ColorRole, Frame, Radius};
use crate::icon_button::{IconButton, IconButtonSize};
use crate::scroll::Scroll;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{DocumentTheme, Theme};

pub const BAR_HEIGHT: f32 = 32.0;
pub const PADDING: f32 = 4.0;
const GAP: f32 = 2.5;
const KEY_SHARE: f32 = 0.8;
const KEY_HEIGHT_MIN: f32 = 40.0;
const KEY_HEIGHT_MAX: f32 = 56.0;
const EMOJI_SIZE: f32 = 44.0;

pub fn keyboard_height(width: f32) -> f32 {
    let key = (width / 10.0 * KEY_SHARE).clamp(KEY_HEIGHT_MIN, KEY_HEIGHT_MAX);
    BAR_HEIGHT + PADDING * 2.0 + key * unstyled::software_keyboard::layout::QWERTY.height()
}

pub fn keys_area(keyboard: Rect) -> Rect {
    let inner = keyboard.shrink(PADDING);
    Rect::from_min_max(
        beui_core::geometry::pos2(inner.left(), inner.top() + BAR_HEIGHT),
        inner.max,
    )
}

pub fn key_center(keyboard: Rect, layer: Layer, value: Value) -> Option<beui_core::geometry::Pos2> {
    let layout = unstyled::software_keyboard::layout::layout(layer);
    let at = layout.find(value)?;
    Some(layout.key_rect(keys_area(keyboard), at)?.center())
}

#[component]
pub fn SoftwareKeyboard() -> NodeId {
    view! {
        <Frame fill=ColorRole::Background padding=Sides::all(PADDING)>
            <List spacing=0.0>
                <KeyboardBar @sizing=ItemSize::Fixed(BAR_HEIGHT) />
                <unstyled::SoftwareKeyboard
                    @sizing=ItemSize::Percent(100.0)
                    cap={|handle: KeyCapHandle| view! {
                        <KeyCap handle />
                    }}
                    emoji={|keyboard: KeyboardHandle| view! {
                        <EmojiPanel keyboard />
                    }}
                />
            </List>
        </Frame>
    }
}

#[component]
fn KeyboardBar() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=0.0 justify=Justify::End align=Align::Center>
            <IconButton
                @test_id="keyboard.close"
                glyph=icons::ICON_KEYBOARD_HIDE
                label="Close the keyboard"
                size=IconButtonSize::Compact
                press_focus=false
                on_click={close_keyboard}
            />
        </List>
    }
}

fn key_fill(pressed: bool, latch: Latch, active: bool, role: Role) -> ColorRole {
    match (pressed, latch, active) {
        (true, _, _) => ColorRole::Pressed,
        (_, Latch::Locked, _) => ColorRole::Accent,
        (_, Latch::Latched, _) | (_, _, true) => ColorRole::AccentSoft,
        _ if role == Role::Normal => ColorRole::SurfaceRaised,
        _ => ColorRole::Surface,
    }
}

#[component]
fn KeyCap(handle: KeyCapHandle) -> NodeId {
    let KeyCapHandle {
        def,
        pressed,
        latch,
        active,
        ..
    } = handle;
    let fill = create_memo(move || {
        key_fill(
            pressed.with(Option::is_some),
            latch.get(),
            active.get(),
            def.role,
        )
    });
    let locked = create_memo(move || latch.get() == Latch::Locked);
    view! {
        <Frame padding=Sides::all(GAP)>
            <Frame fill radius=Radius::Sm>
                <Layers>
                    <Label handle slot=Slot::Center locked align=Align2::CENTER />
                    <Label
                        handle
                        slot=Slot::Nw
                        locked
                        align={Align2::new(Align::Start, Align::Start)}
                    />
                    <Label
                        handle
                        slot=Slot::N
                        locked
                        align={Align2::new(Align::Center, Align::Start)}
                    />
                    <Label
                        handle
                        slot=Slot::Ne
                        locked
                        align={Align2::new(Align::End, Align::Start)}
                    />
                    <Label
                        handle
                        slot=Slot::W
                        locked
                        align={Align2::new(Align::Start, Align::Center)}
                    />
                    <Label
                        handle
                        slot=Slot::E
                        locked
                        align={Align2::new(Align::End, Align::Center)}
                    />
                    <Label
                        handle
                        slot=Slot::Sw
                        locked
                        align={Align2::new(Align::Start, Align::End)}
                    />
                    <Label
                        handle
                        slot=Slot::S
                        locked
                        align={Align2::new(Align::Center, Align::End)}
                    />
                    <Label
                        handle
                        slot=Slot::Se
                        locked
                        align={Align2::new(Align::End, Align::End)}
                    />
                </Layers>
            </Frame>
        </Frame>
    }
}

#[component]
fn Label(handle: KeyCapHandle, slot: Slot, locked: Memo<bool>, align: Align2) -> NodeId {
    let KeyCapHandle {
        values, pressed, ..
    } = handle;
    let center = slot == Slot::Center;
    let shown = create_memo(move || values.get()[slot.index()]);
    let label = create_memo(move || shown.get().map(face).unwrap_or_default());
    let glyph = create_memo(move || label.with(|face: &Face| face.glyph.to_owned()));
    let text = create_memo(move || label.with(|face: &Face| face.text.clone()));
    let chosen =
        create_memo(move || !center && shown.get().is_some() && pressed.get() == Some(shown.get()));
    let color = create_memo(move || match (locked.get(), chosen.get(), center) {
        (true, _, true) => TextColor::OnAccent,
        (_, true, _) => TextColor::Accent,
        (_, _, true) => TextColor::Default,
        _ => TextColor::Muted,
    });
    let size = create_memo(
        move || match (center, text.with(|text| text.chars().count() > 2)) {
            (true, false) => TextSize::Lg,
            (true, true) => TextSize::Sm,
            (false, _) => TextSize::Xs,
        },
    );
    let padding = match center {
        true => Sides::ZERO,
        false => Sides::symmetric(4.0, 1.0),
    };
    view! {
        <Frame align padding>
            <Text string={text} size color ellipsis={!center}>
                <Icon glyph={glyph} size color />
            </Text>
        </Frame>
    }
}

#[component]
fn EmojiPanel(keyboard: KeyboardHandle) -> NodeId {
    let (group, set_group) = create_signal(0usize);
    let shown = create_memo(move || {
        emojis_in(group.get())
            .into_iter()
            .filter(|emoji| drawn(emoji))
            .collect::<Vec<_>>()
    });
    let groups: Vec<usize> = (0..EMOJI_GROUPS.len()).collect();
    view! {
        <List spacing=0.0>
            <List
                direction=Direction::Horizontal
                spacing=0.0
                justify=Justify::SpaceEvenly
                @sizing=ItemSize::Fixed(EMOJI_SIZE)
            >
                <ForEach keys={groups}>
                    {move |index: usize| {
                        let (_, glyph, label) = EMOJI_GROUPS[index];
                        let selected = create_memo(move || group.get() == index);
                        view! {
                            <EmojiTab
                                glyph
                                label
                                selected
                                on_click={move || set_group.set(index)}
                            />
                        }
                    }}
                </ForEach>
            </List>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <List direction=Direction::Horizontal wrap=true spacing=0.0>
                    <ForEach keys={shown}>
                        {move |emoji: &'static str| view! {
                            <EmojiKey emoji keyboard />
                        }}
                    </ForEach>
                </List>
            </Scroll>
            <List direction=Direction::Horizontal spacing=0.0 @sizing=ItemSize::Fixed(EMOJI_SIZE)>
                <PanelKey
                    @sizing=ItemSize::Percent(1.5)
                    keyboard
                    value={Value::Layer(Layer::Text)}
                />
                <PanelKey @sizing=ItemSize::Percent(7.0) keyboard value={Value::Char(' ')} />
                <PanelKey
                    @sizing=ItemSize::Percent(1.5)
                    keyboard
                    value={Value::Key(Key::Backspace)}
                />
            </List>
        </List>
    }
}

fn drawn(text: &str) -> bool {
    layout_text(
        text,
        beui_core::font::FontId::proportional(TextSize::Xl.px()),
        beui_core::font::TextLayout::DEFAULT,
    )
    .is_none_or(|galley| galley.glyphs().iter().all(|glyph| glyph.id.glyph != 0))
}

#[component]
fn EmojiTab(
    glyph: &'static str,
    label: &'static str,
    selected: Memo<bool>,
    on_click: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::Button
            label
            press_focus=false
            tab_stop=false
            on_click={on_click}
            content={move |button: unstyled::ButtonHandle| {
                let fill = create_memo(move || match (selected.get(), button.hovered.get()) {
                    (true, _) => ColorRole::AccentSoft,
                    (false, true) => ColorRole::Hover,
                    (false, false) => ColorRole::None,
                });
                view! {
                    <Frame fill radius=Radius::Sm padding=Sides::all(6.0)>
                        <Text>
                            <Icon glyph size=TextSize::Lg />
                        </Text>
                    </Frame>
                }
            }}
        />
    }
}

#[component]
fn EmojiKey(emoji: &'static str, keyboard: KeyboardHandle) -> NodeId {
    view! {
        <unstyled::Button
            label=emoji
            press_focus=false
            tab_stop=false
            on_click={move || keyboard.type_text(emoji)}
            content={move |button: unstyled::ButtonHandle| {
                let fill = create_memo(move || match button.active.get() {
                    true => ColorRole::Pressed,
                    false => ColorRole::None,
                });
                view! {
                    <Frame
                        width=EMOJI_SIZE
                        height=EMOJI_SIZE
                        align=Align2::CENTER
                        fill
                        radius=Radius::Sm
                    >
                        <Text string=emoji size=TextSize::Xl />
                    </Frame>
                }
            }}
        />
    }
}

#[component]
fn PanelKey(keyboard: KeyboardHandle, value: Value) -> NodeId {
    let shown = face(value);
    view! {
        <unstyled::Button
            label={unstyled::software_keyboard::description(value)}
            press_focus=false
            tab_stop=false
            on_click={move || keyboard.tap(value)}
            content={move |button: unstyled::ButtonHandle| {
                let fill = create_memo(move || match button.active.get() {
                    true => ColorRole::Pressed,
                    false => ColorRole::Surface,
                });
                let Face { glyph, text } = shown.clone();
                view! {
                    <Frame padding=Sides::all(GAP)>
                        <Frame fill radius=Radius::Sm align=Align2::CENTER>
                            <Text string=text size=TextSize::Lg>
                                <Icon glyph size=TextSize::Lg />
                            </Text>
                        </Frame>
                    </Frame>
                }
            }}
        />
    }
}

#[derive(Default)]
pub struct KeyboardWindow {
    document: Option<Document>,
    theme: Option<Theme>,
}

impl KeyboardHost for KeyboardWindow {
    fn height(&mut self, width: f32) -> f32 {
        keyboard_height(width)
    }

    fn show(&mut self, ctx: &Context, rect: Rect, app: &Document) {
        let document = self.document.get_or_insert_with(|| {
            build(|| {
                view! {
                    <SoftwareKeyboard />
                }
            })
        });
        let theme = app.theme();
        if self.theme != Some(theme) {
            self.theme = Some(theme);
            document.set_theme(theme);
        }
        document.show_content(ctx, rect);
    }

    fn closed(&mut self) {
        self.document = None;
        self.theme = None;
    }
}

pub fn install_keyboard(document: &mut Document) {
    document.set_keyboard_host(Box::new(KeyboardWindow::default()));
}
