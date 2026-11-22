use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_view::types::LayerLevel;
use beui_view::types::icons::ICON_X;
use beui_view::types::text::Phrase;

use crate::back_slide::SLIDE_CURVES;
use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::scroll::{Bleed, Scroll};
use crate::sheet::ModalSheet;
use crate::text::{Text, TextStyle};
use crate::theme::{POINTER_REACH, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ActionRole, DialogAction, DialogAnchor, DialogBodyHandle, DialogCardHandle, DialogKind,
    DialogLayout, DialogSheetHandle, DialogSize, ModalHandle, ModalPlacement,
};
use beui_view::primitives::{KeepAlive, Kept, Spacer};
use beui_view::reactive::NodeId;

use crate::list::List;
use crate::tokens::{Length, Padding, Size, Space};
use beui_view::reactive::*;
use beui_view::types::{Align, Direction, Extent, Justify};

pub(crate) const PADDING: Padding = Padding::symmetric(Space::Xl2, Space::Xl);
pub(crate) const SPACING: Space = Space::Lg;
pub const MARGIN: Space = Space::Lg;
const ACTION_SPACING: Space = Space::Md;
pub const DIALOG_WIDTH: Size = Size::Dialog;
pub const SHEET_BELOW: f32 = 400.0;

/// How a modal comes and goes: in a back slide moving on the theme's
/// movements, which carries it away on back, or in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalMotion {
    Slides,
    Still,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scrim {
    Soft,
    Dim,
    Strong,
    Opaque,
}

#[component]
pub fn Modal(
    open: Prop<bool>,
    #[prop(default = Scrim::Dim)] scrim: Scrim,
    #[prop(default = ModalPlacement::Center)] placement: ModalPlacement,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = ModalMotion::Slides)] motion: ModalMotion,
    on_dismiss: Callback<()>,
    #[prop(children)] content: Build<NodeId>,
) -> NodeId {
    let theme = use_theme();
    let color = create_memo(move || match scrim {
        Scrim::Soft => theme.scrim_soft.get(),
        Scrim::Dim => theme.scrim.get(),
        Scrim::Strong => theme.scrim_strong.get(),
        Scrim::Opaque => theme.background.get(),
    });
    let motion = match motion {
        ModalMotion::Slides => unstyled::ModalMotion::Slides(SLIDE_CURVES),
        ModalMotion::Still => unstyled::ModalMotion::Still,
    };
    view! {
        <unstyled::Modal
            open
            scrim={color}
            placement
            level
            motion
            on_dismiss={on_dismiss}
        >|_: ModalHandle| {
            content.call()
        }</unstyled::Modal>
    }
}

#[component]
pub fn Dialog(
    open: Prop<bool>,
    #[prop(default = DialogKind::Dialog)] kind: DialogKind,
    #[prop(default = Phrase::new())] title: Prop<Phrase>,
    #[prop(default = DIALOG_WIDTH)] width: Prop<Size>,
    #[prop(default = DialogSize::Small)] size: DialogSize,
    #[prop(default = DialogAnchor::None)] anchor: Prop<DialogAnchor>,
    #[prop(default = String::new())] id: Prop<String>,
    #[prop(default = Vec::new())] actions: Vec<DialogAction>,
    #[prop(default = true)] scrolls: bool,
    on_dismiss: Callback<()>,
    #[prop(children)] content: Build<Children<ListItem>>,
) -> NodeId {
    view! {
        <DialogFrame open kind width size anchor on_dismiss={on_dismiss}>
            <DialogBody kind title id actions scrolls on_dismiss={on_dismiss}>
                {content.call()}
            </DialogBody>
        </DialogFrame>
    }
}

/// A dialog's outer part. It builds its content once, on first open, and moves
/// it between card and sheet with a `Portal` if the presentation changes while
/// open. Anchored presentations escape a window that has a layer space;
/// centred ones stay in it.
#[component]
pub fn DialogFrame(
    open: Prop<bool>,
    #[prop(default = DialogKind::Dialog)] kind: DialogKind,
    #[prop(default = DIALOG_WIDTH)] width: Prop<Size>,
    #[prop(default = false)] narrow: Prop<bool>,
    #[prop(default = DialogSize::Small)] size: DialogSize,
    #[prop(default = DialogAnchor::None)] anchor: Prop<DialogAnchor>,
    on_dismiss: Callback<()>,
    #[prop(children)] content: Build<NodeId>,
) -> NodeId {
    let theme = use_theme();
    let scrim = create_memo(move || theme.scrim.get());
    view! {
        <unstyled::Dialog
            open
            kind
            scrim={scrim}
            slide=SLIDE_CURVES
            sheet_below=SHEET_BELOW
            narrow
            size
            anchor
            beak=POINTER_REACH
            on_dismiss={on_dismiss}
            card={move |handle: DialogCardHandle| view! {
                <DialogCard width pointer={handle.pointer}>{handle.body}</DialogCard>
            }}
            sheet={|handle: DialogSheetHandle| {
                let DialogSheetHandle { open, close, tall, body } = handle;
                match tall {
                    true => view! {
                        <ModalSheet open rest=1.0 stops={vec![1.0]} on_close={close}>
                            {body}
                        </ModalSheet>
                    },
                    false => view! {
                        <ModalSheet open fit=true on_close={close}>{body}</ModalSheet>
                    },
                }
            }}
            body={move |_: DialogLayout| content.call()}
        />
    }
}

#[component]
pub fn DialogCard(
    #[prop(default = DIALOG_WIDTH)] width: Prop<Size>,
    #[prop(default = NodeRef::new())] pointer: NodeRef,
    children: Child,
) -> NodeId {
    view! {
        <Frame padding=MARGIN>
            <Frame
                @node_ref=&pointer
                width={width.map(at_most)}
                fill=ColorRole::Surface
                border={Border::DEFAULT}
                radius=Radius::Lg
            >
                {children}
            </Frame>
        </Frame>
    }
}

#[component]
pub fn DialogBody(
    #[prop(default = DialogKind::Dialog)] kind: DialogKind,
    #[prop(default = Phrase::new())] title: Prop<Phrase>,
    #[prop(default = String::new())] id: Prop<String>,
    #[prop(default = Vec::new())] actions: Vec<DialogAction>,
    #[prop(default = true)] focuses: Prop<bool>,
    #[prop(default = true)] scrolls: bool,
    on_dismiss: Callback<()>,
    #[prop(children)] content: Build<Children<ListItem>>,
) -> NodeId {
    let id = create_memo(move || id.get());
    let layout = use_context::<DialogLayout>();
    let narrow = create_memo(move || layout.is_some_and(|layout| layout.narrow.get()));
    let inset = create_memo(move || layout.map_or(0.0, |layout| layout.bottom_inset.get()));
    view! {
        <unstyled::DialogBody
            kind
            title
            actions
            narrow
            focuses
            on_dismiss={on_dismiss}
            face={move |handle: DialogBodyHandle| view! {
                <DialogBodyFace handle id scrolls inset>{content.call()}</DialogBodyFace>
            }}
        />
    }
}

fn named(id: Memo<String>, name: &'static str) -> Memo<String> {
    create_memo(move || match id.with(String::is_empty) {
        true => String::new(),
        false => format!("{}.{name}", id.get()),
    })
}

#[component]
fn DialogBodyFace(
    handle: DialogBodyHandle,
    id: Memo<String>,
    scrolls: bool,
    inset: Memo<f32>,
    children: Children<ListItem>,
) -> NodeId {
    let title = handle.title;
    let closable = handle.kind == DialogKind::Dialog;
    let titled = create_memo(move || !title.with(Phrase::is_empty));
    let footer = handle.footer;
    let footed = create_memo(move || !footer.with(unstyled::FooterOrder::is_empty));
    let bleed = create_memo(move || Bleed::dialog(titled.get(), footed.get()));
    let dismiss = handle.dismiss;
    let content = match scrolls {
        true => view! {
            <ListItem shrink=1.0>
                <Scroll fit=true bleed={bleed}>
                    <List spacing=SPACING>{children}</List>
                </Scroll>
            </ListItem>
        },
        false => view! {
            <ListItem grow=1.0>
                <List spacing=SPACING>{children}</List>
            </ListItem>
        },
    };
    let below = create_memo(move || Padding::ZERO.bottom(Space::Custom(inset.get())));
    view! {
        <Frame padding={below} @test_id={id}>
            <Frame padding=PADDING>
                <List spacing=SPACING>
                    <Show value={titled}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <ListItem grow=1.0>
                                <Text style=TextStyle::TITLE>{title}</Text>
                            </ListItem>
                            {closable.then(|| view! {
                                <Button
                                    icon_only=true
                                    glyph=ICON_X
                                    label="Close"
                                    variant=ButtonVariant::Ghost
                                    size=ButtonSize::Inline
                                    @test_id={named(id, "close")}
                                    on_click={dismiss}
                                />
                            })}
                        </List>
                    </Show>
                    {content}
                    <KeepAlive>
                        <Show value={footed}>
                            <Kept>
                                <DialogFooter handle id />
                            </Kept>
                        </Show>
                    </KeepAlive>
                </List>
            </Frame>
        </Frame>
    }
}

#[component]
fn DialogFooter(handle: DialogBodyHandle, id: Memo<String>) -> NodeId {
    let footer = handle.footer;
    let narrow = handle.narrow;
    let start = create_memo(move || footer.get().start);
    let end = create_memo(move || footer.get().end);
    view! {
        <List spacing=Space::None @test_id={named(id, "actions")}>
            <Show
                value={narrow}
                fallback={move || view! {
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        justify=Justify::End
                        wrap=true
                        spacing=ACTION_SPACING
                    >
                        <ForEach value={start}>|index: usize|
                            <ActionButton handle index id />
                        </ForEach>
                        <Show value={move || !start.with(Vec::is_empty)}>
                            <Spacer />
                        </Show>
                        <ForEach value={end}>|index: usize|
                            <ActionButton handle index id />
                        </ForEach>
                    </List>
                }}
            >
                <List spacing=ACTION_SPACING>
                    <ForEach value={end}>|index: usize|
                        <ActionButton handle index id />
                    </ForEach>
                </List>
            </Show>
        </List>
    }
}

fn variant(role: ActionRole) -> ButtonVariant {
    match role {
        ActionRole::Primary => ButtonVariant::Primary,
        ActionRole::Destructive => ButtonVariant::Danger,
        ActionRole::Secondary | ActionRole::Cancel => ButtonVariant::Secondary,
    }
}

#[component]
fn ActionButton(handle: DialogBodyHandle, index: usize, id: Memo<String>) -> NodeId {
    let action = handle.action(index);
    let focus = handle.focus;
    let focused = create_memo(move || focus.get() == Some(index));
    let run = handle.run;
    let narrow = handle.narrow;
    view! {
        <Button
            label={action.label}
            wide={narrow}
            variant={variant(action.role)}
            disabled={action.disabled}
            busy={action.busy}
            focused={focused}
            @test_id={named(id, action.name)}
            on_click={move || run.call(index)}
        />
    }
}

fn at_most(size: Size) -> Length {
    Length::Extent(Extent::at_most(size.px()))
}
