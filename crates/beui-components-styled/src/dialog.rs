use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::icons::ICON_X;
use beui_core::layer::LayerLevel;
use beui_macros::{component, view};

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::scroll::{Bleed, Scroll};
use crate::sheet::ModalSheet;
use crate::text::{Text, TextStyle};
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ActionRole, DialogAction, DialogBodyHandle, DialogKind, DialogLayout, DialogSheetHandle,
    ModalHandle, ModalMotion, ModalPlacement,
};
use beui_core::base::ItemSize;
use beui_core::node::NodeId;
use beui_view::primitives::Spacer;

use crate::list::List;
use crate::theme::Density;
use crate::tokens::{Length, Padding, Size, Space};
use beui_view::reactive::*;

pub(crate) const PADDING: Padding = Padding::symmetric(Space::Xl2, Space::Xl);
pub(crate) const SPACING: Space = Space::Lg;
pub const MARGIN: Space = Space::Lg;
const ACTION_SPACING: Space = Space::Md;
pub const DIALOG_WIDTH: Size = Size::Dialog;
pub const SHEET_BELOW: f32 = 400.0;

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
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    let theme = use_theme();
    let color = create_memo(move || match scrim {
        Scrim::Soft => theme.scrim_soft.get(),
        Scrim::Dim => theme.scrim.get(),
        Scrim::Strong => theme.scrim_strong.get(),
        Scrim::Opaque => theme.background.get(),
    });
    view! {
        <unstyled::Modal
            open
            scrim={color}
            placement
            level
            motion
            on_dismiss={on_dismiss}
        >|_: ModalHandle| {
            content.call(())
        }</unstyled::Modal>
    }
}

#[component]
pub fn Dialog(
    open: Prop<bool>,
    #[prop(default = DialogKind::Dialog)] kind: DialogKind,
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = DIALOG_WIDTH)] width: Prop<Size>,
    #[prop(default = String::new())] id: Prop<String>,
    #[prop(default = Vec::new())] actions: Vec<DialogAction>,
    #[prop(default = true)] scrolls: bool,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render<(), Children<ListChild>>,
) -> NodeId {
    view! {
        <DialogFrame open kind width on_dismiss={on_dismiss}>
            <DialogBody kind title id actions scrolls on_dismiss={on_dismiss}>
                {content.call(())}
            </DialogBody>
        </DialogFrame>
    }
}

#[component]
pub fn DialogFrame(
    open: Prop<bool>,
    #[prop(default = DialogKind::Dialog)] kind: DialogKind,
    #[prop(default = DIALOG_WIDTH)] width: Prop<Size>,
    #[prop(default = false)] narrow: Prop<bool>,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    let theme = use_theme();
    let scrim = create_memo(move || theme.scrim.get());
    view! {
        <unstyled::Dialog
            open
            kind
            scrim={scrim}
            sheet_below=SHEET_BELOW
            narrow
            on_dismiss={on_dismiss}
            card={move |body: Child| view! {
                <DialogCard width>{body}</DialogCard>
            }}
            sheet={|handle: DialogSheetHandle| {
                let DialogSheetHandle { open, close, body } = handle;
                view! {
                    <ModalSheet open fit=true on_close={close}>{body}</ModalSheet>
                }
            }}
            body={move |_: DialogLayout| content.call(())}
        />
    }
}

#[component]
pub fn DialogCard(#[prop(default = DIALOG_WIDTH)] width: Prop<Size>, children: Child) -> NodeId {
    view! {
        <Frame padding=MARGIN>
            <Frame
                width={width.map(at_most)}
                fill=ColorRole::SurfaceRaised
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
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = String::new())] id: Prop<String>,
    #[prop(default = Vec::new())] actions: Vec<DialogAction>,
    #[prop(default = true)] focuses: Prop<bool>,
    #[prop(default = true)] scrolls: bool,
    on_dismiss: ClickCallback,
    #[prop(children)] content: Render<(), Children<ListChild>>,
) -> NodeId {
    let id = create_memo(move || id.get());
    let layout = use_context::<DialogLayout>();
    let narrow = create_memo(move || layout.is_some_and(|layout| layout.narrow.get()));
    view! {
        <unstyled::DialogBody
            kind
            title
            actions
            narrow
            focuses
            on_dismiss={on_dismiss}
            face={move |handle: DialogBodyHandle| view! {
                <DialogBodyFace handle id scrolls>{content.call(())}</DialogBodyFace>
            }}
        />
    }
}

fn named(id: Memo<String>, name: &'static str) -> Memo<String> {
    create_memo(move || match id.get().is_empty() {
        true => String::new(),
        false => format!("{}.{name}", id.get()),
    })
}

#[component]
fn DialogBodyFace(
    handle: DialogBodyHandle,
    id: Memo<String>,
    scrolls: bool,
    children: Children<ListChild>,
) -> NodeId {
    let title = handle.title;
    let closable = handle.kind == DialogKind::Dialog;
    let titled = create_memo(move || !title.get().is_empty());
    let footer = handle.footer;
    let footed = create_memo(move || !footer.get().is_empty());
    let bleed = create_memo(move || Bleed::dialog(titled.get(), footed.get()));
    let dismiss = handle.dismiss;
    let content = match scrolls {
        true => view! {
            <Scroll @sizing={ItemSize::Intrinsic.shrink(1.0)} fit=true bleed={bleed}>
                <List spacing=SPACING>{children}</List>
            </Scroll>
        },
        false => view! {
            <List @sizing=ItemSize::Percent(100.0) spacing=SPACING>{children}</List>
        },
    };
    view! {
        <Frame padding=PADDING>
            <List spacing=SPACING>
                <Show condition={titled}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::TITLE>{title}</Text>
                        <Show condition={create_memo(move || closable)}>
                            <Button
                                icon_only=true
                                glyph=ICON_X
                                label="Close"
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Small
                                @test_id={named(id, "close")}
                                on_click={dismiss}
                            />
                        </Show>
                    </List>
                </Show>
                {content}
                <ShowKeepAlive condition={footed}>
                    <DialogFooter handle id />
                </ShowKeepAlive>
            </List>
        </Frame>
    }
}

#[component]
fn DialogFooter(handle: DialogBodyHandle, id: Memo<String>) -> NodeId {
    let footer = handle.footer;
    let narrow = handle.narrow;
    let wide = create_memo(move || !narrow.get());
    let start = create_memo(move || footer.get().start);
    let end = create_memo(move || footer.get().end);
    let leading = create_memo(move || !start.with(Vec::is_empty));
    view! {
        <List spacing=Space::None @test_id={named(id, "actions")}>
            <Show condition={wide}>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::End
                    wrap=true
                    spacing=ACTION_SPACING
                >
                    <ForEach keys={start}>|index: usize|
                        <ActionButton handle index id />
                    </ForEach>
                    <Show condition={leading}>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </Show>
                    <ForEach keys={end}>|index: usize|
                        <ActionButton handle index id />
                    </ForEach>
                </List>
            </Show>
            <Show condition={narrow}>
                <List spacing=ACTION_SPACING>
                    <ForEach keys={end}>|index: usize|
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
    Length::Extent(Extent::at_most(size.resolve(Density::Compact)))
}
