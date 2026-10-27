use crate::unstyled as keyboard;
use crate::unstyled::{
    EMOJI_GROUPS, Face, KeyCapHandle, KeyboardHandle, Latch, Layer, Preview, Role, Slot, Value,
    composed_face, emojis_in, face,
};
use beui_components_unstyled as unstyled;
use beui_core::current::{close_keyboard, layout_text};
use beui_core::font::Overflow;
use beui_core::geometry::Rect;
use beui_core::icons;
use beui_core::input::Key;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::Layers;
use beui_view::reactive::*;

use beui_components_styled::List;
use beui_components_styled::border::Separator;
use beui_components_styled::button::{Button, ButtonSize};
use beui_components_styled::frame::{ColorRole, Frame, Radius};
use beui_components_styled::scroll::Scroll;
use beui_components_styled::text::{Icon, Text, TextColor, TextSize};
use beui_components_styled::tokens::{Padding, Size, Space, Stroke};
use beui_view::types::{Align, Align2, Direction, ItemSize, Justify};

pub const BAR_HEIGHT: f32 = 32.0;
pub const PADDING: Space = Space::Xs;
const GAP: Space = Space::Xs2;
const EMOJI_SIZE: f32 = 44.0;

pub fn keys_area(keyboard: Rect) -> Rect {
    let inner = keyboard.shrink(PADDING.px());
    Rect::from_min_max(
        beui_core::geometry::pos2(
            inner.left(),
            inner.top() + Stroke::Hairline.px() + BAR_HEIGHT,
        ),
        inner.max,
    )
}

pub fn key_center(keyboard: Rect, layer: Layer, value: Value) -> Option<beui_core::geometry::Pos2> {
    let layout = crate::unstyled::layout::layout(layer);
    let at = layout.find(value)?;
    Some(layout.key_rect(keys_area(keyboard), at)?.center())
}

pub fn close_center(keyboard: Rect) -> beui_core::geometry::Pos2 {
    let inner = keyboard.shrink(PADDING.px());
    beui_core::geometry::pos2(
        inner.right() - Size::Toolbar.px() / 2.0,
        inner.top() + Stroke::Hairline.px() + BAR_HEIGHT / 2.0,
    )
}

#[component]
pub fn SoftwareKeyboard() -> NodeId {
    view! {
        <List spacing=Space::None>
            <Separator />
            <Frame @sizing=ItemSize::Percent(100.0) fill=ColorRole::Background padding=PADDING>
                <List spacing=Space::None>
                    <KeyboardBar @sizing=ItemSize::Fixed(BAR_HEIGHT) />
                    <keyboard::SoftwareKeyboard
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
        </List>
    }
}

#[component]
fn KeyboardBar() -> NodeId {
    view! {
        <List
            direction=Direction::Horizontal
            spacing=Space::None
            justify=Justify::End
            align=Align::Center
        >
            <Button
                icon_only=true
                @test_id="keyboard.close"
                glyph=icons::ICON_KEYBOARD_OFF
                label="Close the keyboard"
                size=ButtonSize::Toolbar
                press_focus=false
                on_click={close_keyboard}
            />
        </List>
    }
}

fn key_fill(pressed: bool, latch: Latch, active: bool, faded: bool, role: Role) -> ColorRole {
    match (pressed, latch, active) {
        (true, _, _) => ColorRole::Pressed,
        _ if faded => ColorRole::None,
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
        values,
        previews,
        pressed,
        latch,
        active,
        ..
    } = handle;
    let faded = create_memo(move || {
        let values = values.get();
        let previews = previews.get();
        values.iter().any(Option::is_some)
            && values
                .iter()
                .zip(previews)
                .all(|(value, preview)| value.is_none() || preview == Preview::Faded)
    });
    let fill = create_memo(move || {
        key_fill(
            pressed.with(Option::is_some),
            latch.get(),
            active.get(),
            faded.get(),
            def.role,
        )
    });
    let locked = create_memo(move || latch.get() == Latch::Locked);
    view! {
        <Frame padding=GAP>
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
        values,
        previews,
        pressed,
        ..
    } = handle;
    let center = slot == Slot::Center;
    let shown = create_memo(move || values.get()[slot.index()]);
    let preview = create_memo(move || previews.get()[slot.index()]);
    let label = create_memo(move || match preview.get() {
        Preview::Becomes(text) => composed_face(text),
        Preview::Same | Preview::Faded => shown.get().map(face).unwrap_or_default(),
    });
    let glyph = create_memo(move || label.with(|face: &Face| face.glyph.to_owned()));
    let text = create_memo(move || label.with(|face: &Face| face.text.clone()));
    let after = create_memo(move || label.with(|face: &Face| face.after.clone()));
    let chosen =
        create_memo(move || !center && shown.get().is_some() && pressed.get() == Some(shown.get()));
    let color = create_memo(move || match (locked.get(), chosen.get(), center) {
        _ if preview.get() == Preview::Faded => TextColor::Faint,
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
        true => Padding::ZERO,
        false => Padding::symmetric(Space::Xs, Space::Xs2),
    };
    view! {
        <Frame align padding>
            <Text size color overflow=Overflow::Ellipsis>
                {text}
                <Icon glyph={glyph} size color />
                {after}
            </Text>
        </Frame>
    }
}

#[component]
fn EmojiPanel(keyboard: KeyboardHandle) -> NodeId {
    let group = create_signal(0usize);
    let shown = create_memo(move || {
        emojis_in(group.get())
            .into_iter()
            .filter(|emoji| drawn(emoji))
            .collect::<Vec<_>>()
    });
    let groups: Vec<usize> = (0..EMOJI_GROUPS.len()).collect();
    view! {
        <List spacing=Space::None>
            <List
                direction=Direction::Horizontal
                spacing=Space::None
                justify=Justify::SpaceEvenly
                @sizing=ItemSize::Fixed(EMOJI_SIZE)
            >
                <ForEach keys={groups}>|index: usize| {
                    {
                        let (_, emoji, label) = EMOJI_GROUPS[index];
                        let selected = create_memo(move || group.get() == index);
                        view! {
                            <EmojiTab emoji label selected on_click={move || group.set(index)} />
                        }
                    }
                }</ForEach>
            </List>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <List direction=Direction::Horizontal wrap=true spacing=Space::None>
                    <ForEach keys={shown}>|emoji: &'static str|
                        <EmojiKey emoji keyboard />
                    </ForEach>
                </List>
            </Scroll>
            <List
                direction=Direction::Horizontal
                spacing=Space::None
                @sizing=ItemSize::Fixed(EMOJI_SIZE)
            >
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
    emoji: &'static str,
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
                    <Frame fill radius=Radius::Sm padding=Space::Sm>
                        <Text size=TextSize::Lg>{emoji}</Text>
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
                        width={Size::Custom(EMOJI_SIZE)}
                        height={Size::Custom(EMOJI_SIZE)}
                        align=Align2::CENTER
                        fill
                        radius=Radius::Sm
                    >
                        <Text size=TextSize::Xl>{emoji}</Text>
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
            label={crate::unstyled::description(value)}
            press_focus=false
            tab_stop=false
            on_click={move || keyboard.tap(value)}
            content={move |button: unstyled::ButtonHandle| {
                let fill = create_memo(move || match button.active.get() {
                    true => ColorRole::Pressed,
                    false => ColorRole::Surface,
                });
                let Face { glyph, text, .. } = shown.clone();
                view! {
                    <Frame padding=GAP>
                        <Frame fill radius=Radius::Sm align=Align2::CENTER>
                            <Text size=TextSize::Lg>
                                {text}
                                <Icon glyph size=TextSize::Lg />
                            </Text>
                        </Frame>
                    </Frame>
                }
            }}
        />
    }
}
