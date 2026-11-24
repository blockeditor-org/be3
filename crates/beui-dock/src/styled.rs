use beui_components_styled::button::{ButtonSize, ButtonVariant};
use beui_components_styled::frame::{ColorRole, Frame as PanelFrame};
use beui_components_styled::menu_button::MenuButton;
use beui_components_styled::scroll::Scroll;
use beui_components_styled::tokens::Space;
use beui_components_styled::tool_pill::ToolPill;
use beui_components_unstyled::menu::MenuNodes;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::icons::{ICON_GRIP_HORIZONTAL, ICON_GRIP_VERTICAL};
use beui_view::types::{Direction, Placement, PointerPress, Rect};

use crate::{DockBarControl, use_dock_bar};

/// The standard panel a tab's body is in a dock with a stage: a `Surface` fill, a `Scroll` filling
/// it, and padding inside that, around the content.
#[component]
pub(crate) fn DockPanel(content: BuildFn<NodeId>) -> NodeId {
    view! {
        <PanelFrame fill=ColorRole::Surface>
            <Scroll>
                <PanelFrame padding=Space::Lg>{content.call()}</PanelFrame>
            </Scroll>
        </PanelFrame>
    }
}

#[component]
pub fn DockBarSurface(
    #[prop(default = Space::Xs2)] spacing: Prop<Space>,
    children: Children<ListItem>,
) -> NodeId {
    let bar = use_dock_bar();
    let surface = bar.map_or_else(NodeRef::new, |bar| bar.surface());
    view! {
        <ToolPill
            @node_ref=&surface
            direction={move || bar.map_or(Direction::Horizontal, |bar| bar.direction())}
            band={move || bar.is_some_and(|bar| bar.placement().is_beside())}
            spacing={spacing}
            children
        />
    }
}

#[component]
pub fn DockBarGrip(
    control: Option<DockBarControl>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
) -> NodeId {
    let Some(control) = control.or_else(use_dock_bar) else {
        return view! {
            <Frame />
        };
    };
    let pressed = StoredCell::new(None::<PointerPress>);
    let moved = StoredCell::new(false);
    let menu = control.menu();
    let open = create_signal(false);
    // A local bar lifts onto the drag board, so the dock's bands and the keys reach it; a
    // hosted one, or one the dock moves another way, goes on as `on_drag`.
    let drag = Func::new(move |press: PointerPress| {
        let carry = control.carry(pressed.get().unwrap_or(press));
        if carry.is_some() {
            moved.set(true);
        }
        carry
    });
    view! {
        <Interactive
            cursor=beui_view::types::CursorIcon::Grab
            drag={Some(drag)}
            on_press={move |press: PointerPress| {
                pressed.set(Some(press));
                moved.set(false);
            }}
            on_drag={move |press: PointerPress| {
                let Some(down) = pressed.get() else {
                    return;
                };
                if moved.get() {
                    control.begin_move(press);
                } else if (press.pos - down.pos).length() >= beui_view::types::DRAG_THRESHOLD {
                    moved.set(true);
                    control.begin_move(down);
                    control.begin_move(press);
                }
            }}
            on_active_change={move |active: bool| {
                if !active {
                    pressed.set(None);
                    moved.set(false);
                    control.end_move();
                }
            }}
        >
            <MenuButton
                @test_id={format!("dock.bar.{}.grip", control.bar().value())}
                label="Move toolbar"
                glyph={move || {
                    Some(match control.direction() {
                        Direction::Horizontal => ICON_GRIP_VERTICAL,
                        Direction::Vertical => ICON_GRIP_HORIZONTAL,
                    })
                }}
                icon_only=true
                arrow=false
                variant=ButtonVariant::Ghost
                size=ButtonSize::Toolbar
                open
                bounds
                placement=Placement::BELOW_START
                click_opens=false
                secondary_opens=true
                on_click={move || {
                    if !moved.get() {
                        open.update(|open| *open = !*open);
                    }
                }}
                items={view! {
                    <MenuNodes nodes={menu} test_prefix={"grip".to_owned()} />
                }}
            />
        </Interactive>
    }
}
