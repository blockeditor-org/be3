use crate::frame::{Border, ColorRole, Frame, Radius};
use accesskit::{Node, Role};
use beui_core::layer::Dismiss;
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::scroll::{Bleed, Scroll};
use crate::text::{Text, TextStyle};
use beui_components_unstyled::BackSlide;
use beui_core::base::ItemSize;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::geometry::Pos2;
use beui_core::input::Key;
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub(crate) const PADDING_HORIZONTAL: f32 = 20.0;
pub(crate) const PADDING_VERTICAL: f32 = 18.0;
pub(crate) const SPACING: f32 = 12.0;
pub const MARGIN: f32 = 12.0;
const ACTION_SPACING: f32 = 8.0;

#[component]
pub fn Dialog(
    open: Prop<bool>,
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = 360.0)] width: Prop<f32>,
    #[prop(default = String::new())] id: Prop<String>,
    #[prop(default = String::new())] confirm: Prop<String>,
    #[prop(default = String::new())] cancel: Prop<String>,
    #[prop(default = false)] destructive: Prop<bool>,
    #[prop(default = false)] confirm_disabled: Prop<bool>,
    on_confirm: ClickCallback,
    on_dismiss: ClickCallback,
    extra_actions: Option<Render<(), Children<ListChild>>>,
    #[prop(children)] content: Render<(), Children<ListChild>>,
) -> NodeId {
    let presence = create_signal(1.0_f32);
    let theme = crate::theme::use_theme();
    let scrim = create_memo(move || theme.scrim.get().scale_alpha(presence.get()));
    let actions = DialogActions {
        id: create_memo(move || id.get()),
        confirm: create_memo(move || confirm.get()),
        cancel: create_memo(move || cancel.get()),
        destructive: create_memo(move || destructive.get()),
        confirm_disabled: create_memo(move || confirm_disabled.get()),
        focus_confirm: create_memo(|| false),
        on_confirm,
        on_cancel: on_dismiss,
    };
    let body = Render::new(move |()| {
        view! {
            <BackSlide enters=false on_back={on_dismiss} on_presence={presence}>
                <DialogSurface
                    width={width}
                    title={title}
                    role=Role::Dialog
                    actions={actions}
                    extra={ExtraActions(extra_actions)}
                >
                    {content.call(())}
                </DialogSurface>
            </BackSlide>
        }
    })
    .once();
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={open}
            keep_alive=true
            placement=Placement::Center
            scrim={scrim}
            on_dismiss={move |_: Dismiss| on_dismiss.call()}
            content={move || body.call(())}
        />
    }
}

#[derive(Clone, Copy)]
pub(crate) struct DialogActions {
    pub(crate) id: Memo<String>,
    pub(crate) confirm: Memo<String>,
    pub(crate) cancel: Memo<String>,
    pub(crate) destructive: Memo<bool>,
    pub(crate) confirm_disabled: Memo<bool>,
    pub(crate) focus_confirm: Memo<bool>,
    pub(crate) on_confirm: ClickCallback,
    pub(crate) on_cancel: ClickCallback,
}

impl DialogActions {
    fn shown(&self) -> Memo<bool> {
        let confirm = self.confirm;
        let cancel = self.cancel;
        create_memo(move || !confirm.get().is_empty() || !cancel.get().is_empty())
    }

    fn confirm(&self) {
        match self.on_confirm.is_empty() {
            true => self.on_cancel.call(),
            false => self.on_confirm.call(),
        }
    }

    fn enter(&self) -> bool {
        if self.confirm.get_untracked().is_empty()
            || self.destructive.get_untracked()
            || self.confirm_disabled.get_untracked()
        {
            return false;
        }
        self.confirm();
        true
    }
}

#[component]
pub(crate) fn DialogSurface(
    width: Prop<f32>,
    title: Prop<String>,
    role: Role,
    actions: DialogActions,
    #[prop(default = ExtraActions(None))] extra: ExtraActions,
    #[prop(default = String::new())] surface_id: String,
    children: Children<ListChild>,
) -> NodeId {
    let label = create_memo(move || title.get());
    let titled = create_memo(move || !label.get().is_empty());
    let footed = actions.shown();
    let bleed = create_memo(move || match footed.get() {
        true => Bleed::DIALOG_ABOVE_ACTIONS,
        false => Bleed::DIALOG,
    });
    component_accessibility(create_memo(move || {
        let mut node = Node::new(role);
        let title = label.get();
        if !title.is_empty() {
            node.set_label(title);
        }
        node
    }));
    let enter = actions;
    on_key(
        KeyAt::Node(beui_core::tree::component_node()),
        KeyPhase::Bubble,
        move |event| {
            event.press().is_some_and(|press| {
                press.key == Key::Enter
                    && press.pressed
                    && !press.repeat
                    && !press.modifiers.ctrl
                    && !press.modifiers.alt
                    && !press.modifiers.shift
                    && enter.enter()
            })
        },
    );
    view! {
        <Frame padding=MARGIN>
            <Frame
                width={width.map(Extent::at_most)}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Lg
                padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
                @test_id=surface_id
            >
                <List spacing=0.0>
                    <Scroll @sizing={ItemSize::Intrinsic.shrink(1.0)} fit=true bleed={bleed}>
                        <List spacing=SPACING>
                            <Show condition={titled}>
                                <Text style=TextStyle::TITLE>{label}</Text>
                            </Show>
                            {children}
                        </List>
                    </Scroll>
                    <ShowKeepAlive condition={footed}>
                        <Frame padding={Sides::new(0.0, SPACING, 0.0, 0.0)}>
                            <DialogFooter actions={actions} extra={extra} />
                        </Frame>
                    </ShowKeepAlive>
                </List>
            </Frame>
        </Frame>
    }
}

pub(crate) struct ExtraActions(Option<Render<(), Children<ListChild>>>);

#[component]
fn DialogFooter(actions: DialogActions, extra: ExtraActions) -> NodeId {
    let ExtraActions(extra) = extra;
    let extra = extra.map(|extra| {
        view! {
            <List direction=Direction::Horizontal align=Align::Center spacing=ACTION_SPACING>
                {extra.call(())}
            </List>
        }
    });
    let spacer = extra.is_some().then(|| {
        view! {
            <Spacer @sizing=ItemSize::Percent(100.0) />
        }
    });
    let confirming = create_memo(move || !actions.confirm.get().is_empty());
    let cancelling = create_memo(move || !actions.cancel.get().is_empty());
    let confirm_variant = create_memo(move || match actions.destructive.get() {
        true => ButtonVariant::Danger,
        false => ButtonVariant::Primary,
    });
    let id = actions.id;
    let named = move |name: &'static str| {
        create_memo(move || match id.get().is_empty() {
            true => String::new(),
            false => format!("{}.{name}", id.get()),
        })
    };
    let cancel_id = named("cancel");
    let footer_id = named("actions");
    let confirm_id = named("confirm");
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            justify=Justify::End
            wrap=true
            spacing=ACTION_SPACING
            @test_id=footer_id
        >
            {extra}
            {spacer}
            <Show condition={cancelling}>
                <Button
                    label={actions.cancel}
                    variant=ButtonVariant::Secondary
                    @test_id={cancel_id}
                    on_click={actions.on_cancel}
                />
            </Show>
            <Show condition={confirming}>
                <Button
                    label={actions.confirm}
                    variant={confirm_variant}
                    disabled={actions.confirm_disabled}
                    focused={actions.focus_confirm}
                    @test_id={confirm_id}
                    on_click={move || actions.confirm()}
                />
            </Show>
        </List>
    }
}
