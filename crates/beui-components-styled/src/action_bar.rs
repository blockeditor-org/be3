use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ActionBarHandle, MenuNodes};
use beui_core::icons::ICON_ELLIPSIS_VERTICAL;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::actions::{Action, MenuNode};
use beui_view::primitives::*;
use beui_view::reactive::*;

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::frame::{ColorRole, Frame};
use crate::menu_button::MenuButton;

const SPACING: f32 = 2.0;
const DIVIDER_HEIGHT: f32 = 18.0;
const DIVIDER_MARGIN: f32 = 4.0;
const RULE: f32 = 1.0;
const LABEL_MARGIN: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionLabels {
    #[default]
    Shown,
    Icons,
    Overflow,
}

#[component]
pub fn ActionBar(
    menu: Prop<Vec<MenuNode>>,
    #[prop(default = ActionLabels::Shown)] labels: ActionLabels,
    #[prop(default = false)] title: Prop<bool>,
    #[prop(default = Direction::Horizontal)] direction: Direction,
    #[prop(default = ButtonSize::Toolbar)] size: ButtonSize,
    #[prop(default = String::from("actions"))] test_prefix: String,
) -> NodeId {
    view! {
        <unstyled::ActionBar
            menu
            labels={labels != ActionLabels::Overflow}
            title
        >|handle: ActionBarHandle| <ActionBarFace handle labels direction size test_prefix /></unstyled::ActionBar>
    }
}

#[component]
fn ActionBarFace(
    handle: ActionBarHandle,
    labels: ActionLabels,
    direction: Direction,
    size: ButtonSize,
    test_prefix: String,
) -> NodeId {
    let ActionBarHandle {
        icons,
        labels: labelled,
        overflow,
    } = handle;
    let keys = |actions: Memo<Vec<Action>>| {
        create_memo(move || actions.with(|actions| actions.iter().map(Action::key).collect::<Vec<_>>()))
    };
    let (icon_keys, label_keys) = (keys(icons), keys(labelled));
    let divided = create_memo(move || {
        labels == ActionLabels::Shown
            && icons.with(|icons| !icons.is_empty())
            && labelled.with(|labelled| !labelled.is_empty())
    });
    let offered = create_memo(move || overflow.with(|items| !items.is_empty()));
    let prefix = test_prefix.clone();
    let pin = move |actions: Memo<Vec<Action>>, key: u64, shown: bool| {
        let action = actions.with_untracked(|actions| {
            actions.iter().find(|action| action.key() == key).cloned()
        });
        match action {
            Some(action) => view! {
                <ActionPin action labelled=shown size prefix={prefix.clone()} />
            },
            None => view! {
                <Frame />
            },
        }
    };
    let icon_pin = pin.clone();
    let labelled_pin = pin;
    let menu_id = format!("{test_prefix}.menu");
    view! {
        <List direction=direction align=Align::Center spacing=SPACING>
            <ForEach keys={icon_keys}>|key: u64| { icon_pin(icons, key, false) }</ForEach>
            <Show condition={divided}>
                <Frame padding={Sides::horizontal(DIVIDER_MARGIN)}>
                    <Frame width=RULE height=DIVIDER_HEIGHT fill=ColorRole::Border />
                </Frame>
            </Show>
            <ForEach keys={label_keys}>|key: u64| {
                labelled_pin(labelled, key, labels == ActionLabels::Shown)
            }</ForEach>
            <Show condition={offered}>
                <MenuButton
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    @test_id={menu_id.clone()}
                    label="More"
                    glyph=ICON_ELLIPSIS_VERTICAL
                    size
                    items={view! {
                        <MenuNodes nodes={overflow} test_prefix={menu_id.clone()} />
                    }}
                />
            </Show>
        </List>
    }
}

#[component]
fn ActionPin(action: Action, labelled: bool, size: ButtonSize, prefix: String) -> NodeId {
    let id = format!("{prefix}.pin.{}", action.id());
    match labelled {
        true => view! {
            <Frame padding={Sides::horizontal(LABEL_MARGIN)}>
                <Button
                    @test_id={id}
                    action={action}
                    variant=ButtonVariant::Primary
                    size=ButtonSize::Compact
                />
            </Frame>
        },
        false => view! {
            <Button
                icon_only=true
                @test_id={id}
                action={action}
                variant=ButtonVariant::Ghost
                size
            />
        },
    }
}
