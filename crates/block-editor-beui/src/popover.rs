use beui::reactive::*;
use beui::styled::theme::POINTER_REACH;
use beui::styled::*;
use beui::{
    Dismiss, Key, KeyAt, KeyPhase, LayerPolicy, NodeId, OverlayAnchor, Placement, Pos2, Rect, Vec2,
};
use block_plugin_api::{ChildLayer, ChildMode};

use crate::{ChildState, Editor, PickAnchor, Subregion, SubregionContent};

const GAP: u16 = 4;
const MARGIN: u16 = 8;
const INSET: Space = Space::Sm;

#[derive(Clone, Copy)]
pub(crate) struct PopoverWindow(pub(crate) Memo<Option<beui::WindowId>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    Bare,
    Card,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shown {
    Anchored,
    Sheet,
}

#[component]
pub(crate) fn ChildPopover(
    editor: Editor,
    content: SubregionContent,
    anchor: PickAnchor,
    shown: Shown,
    face: Memo<Face>,
    size: Memo<Vec2>,
    testing: &'static str,
    on_state: Callback<ChildState>,
    on_dismiss: ClickCallback,
) -> NodeId {
    let window = match anchor {
        PickAnchor::Node(node) => node.try_get().map(window_of),
        PickAnchor::Centre | PickAnchor::Rect(_) => None,
    }
    .or_else(|| editor.content_window());
    let child = view! {
        <Subregion
            editor
            placed={Some(content)}
            mode=ChildMode::Live
            layer=ChildLayer::Below
            @test_id={testing}
            on_state={move |state: ChildState| on_state.call(state)}
        />
    };
    let height = create_memo(move || Size::Custom(size.get().y));
    let inner = match shown {
        Shown::Anchored => view! {
            <AnchoredPopover window anchor face child size on_dismiss />
        },
        Shown::Sheet => view! {
            <ModalSheet open=true fit=true on_close={move || on_dismiss.call()}>
                <Frame width=Length::FILL height>{child}</Frame>
            </ModalSheet>
        },
    };
    view! {
        <Overlay
            anchor={OverlayAnchor::Point(Pos2::ZERO)}
            placement=Placement::At
            policy=LayerPolicy::Inert
            traps_focus=false
            dismissable=false
            shown_in={window}
            open=true
            on_dismiss={|_: Dismiss| {}}
        >
            {inner}
        </Overlay>
    }
}

#[component]
fn AnchoredPopover(
    window: Option<beui::WindowId>,
    anchor: PickAnchor,
    face: Memo<Face>,
    child: NodeId,
    size: Memo<Vec2>,
    on_dismiss: ClickCallback,
) -> NodeId {
    let viewport = create_memo(move || {
        beui::native::try_with_document(|document| {
            window
                .and_then(|window| document.window_rect(window))
                .unwrap_or_else(|| document.viewport_rect())
        })
        .map_or(Rect::ZERO, |rect| {
            Rect::from_min_size(Pos2::ZERO, rect.size())
        })
    });
    let room = create_memo(move || {
        let area = beui::native::try_with_document(|document| {
            document
                .context()?
                .layer_space(window.unwrap_or(beui::WindowId::MAIN))
                .map(|space| space.area())
        })
        .flatten()
        .unwrap_or_else(|| viewport.get());
        let margins = 2.0 * f32::from(MARGIN);
        let anchored = match anchor {
            PickAnchor::Node(node) => crate::node_anchor(&node),
            PickAnchor::Rect(rect) => Some(rect),
            PickAnchor::Centre => None,
        };
        let tall = anchored.map_or(area.height(), |anchored| {
            (area.max.y - anchored.max.y).max(anchored.min.y - area.min.y)
        });
        Vec2::new(area.width() - margins, tall - margins)
    });
    let shown = create_memo(move || size.get().min(room.get()).max(Vec2::ZERO));
    let width = create_memo(move || Size::Custom(shown.get().x));
    let height = create_memo(move || Size::Custom(shown.get().y));
    let carded = create_memo(move || face.get() == Face::Card);
    let placed = create_memo(move || {
        let placement = match carded.get() {
            true => Placement::Popover {
                gap: GAP,
                margin: MARGIN,
            },
            false => Placement::BelowStart,
        };
        match anchor {
            PickAnchor::Node(node) => (OverlayAnchor::Node(node), placement, carded.get()),
            PickAnchor::Rect(rect) => (OverlayAnchor::Rect(rect), placement, carded.get()),
            PickAnchor::Centre => (
                OverlayAnchor::Rect(viewport.get()),
                Placement::CenterOn,
                false,
            ),
        }
    });
    let at = create_memo(move || placed.get().0);
    let placement = create_memo(move || placed.get().1);
    let beak = create_memo(move || match placed.get().2 {
        true => POINTER_REACH,
        false => 0.0,
    });
    let fill = create_memo(move || match carded.get() {
        true => ColorRole::SurfaceRaised,
        false => ColorRole::None,
    });
    let border = create_memo(move || carded.get().then_some(Border::DEFAULT));
    let padding = create_memo(move || match carded.get() {
        true => Padding::all(INSET),
        false => Padding::ZERO,
    });
    let pointer = NodeRef::new();
    on_key(KeyAt::Host, KeyPhase::Capture, move |event| {
        let escaped = event
            .press()
            .is_some_and(|press| press.pressed && press.key == Key::Escape);
        if escaped {
            on_dismiss.call();
        }
        escaped
    });
    view! {
        <Overlay
            anchor={at}
            placement
            policy=LayerPolicy::Pass
            traps_focus=false
            pointer={pointer}
            beak
            open=true
            on_dismiss={move |why: Dismiss| {
                if matches!(why, Dismiss::OutsidePress | Dismiss::Escape | Dismiss::Back) {
                    on_dismiss.call();
                }
            }}
        >
            <Frame @node_ref=&pointer fill border radius=Radius::Lg padding>
                <Frame width height>{child}</Frame>
            </Frame>
        </Overlay>
    }
}

const RENAME_SIZE: Vec2 = Vec2::new(392.0, 560.0);
const MENU_SIZE: Vec2 = Vec2::new(256.0, 320.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Rename(uuid::Uuid),
    AppMenu,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Popped {
    pub(crate) serial: u64,
    pub(crate) asked: Asked,
    pub(crate) anchor: PickAnchor,
    pub(crate) touch: bool,
}

#[component]
pub(crate) fn Popovers(editor: Editor) -> NodeId {
    let popped = editor.popped();
    let keys = create_memo(move || {
        popped
            .with(|popped| popped.map(|popped| popped.serial))
            .into_iter()
            .collect::<Vec<_>>()
    });
    view! {
        <Layers>
            <ForEach keys>|serial: u64| {
                match popped.get_untracked().filter(|popped| popped.serial == serial) {
                    Some(popped) => view! {
                        <PoppedChild editor popped />
                    },
                    None => view! {
                        <Frame />
                    },
                }
            }</ForEach>
        </Layers>
    }
}

#[component]
fn PoppedChild(editor: Editor, popped: Popped) -> NodeId {
    let touch = popped.touch || crate::phone_layout().get_untracked();
    let (content, face, first, testing) = match popped.asked {
        Asked::Rename(block) => (
            SubregionContent::Rename(block),
            Face::Card,
            RENAME_SIZE,
            "rename.child",
        ),
        Asked::AppMenu => (
            SubregionContent::Host(block_plugin_api::HostPanel::AppMenu),
            Face::Bare,
            MENU_SIZE,
            "app-menu.child",
        ),
    };
    let size = create_signal(first);
    let serial = popped.serial;
    let on_state = Callback::new(move |state: ChildState| {
        if let Some(wanted) = state
            .intrinsic_size
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            && size.get_untracked() != wanted
        {
            size.set(wanted);
        }
        if state.dialog.answer.is_some() {
            editor.close_popped(serial);
        }
    });
    view! {
        <ChildPopover
            editor
            content
            anchor={popped.anchor}
            shown={match touch {
                true => Shown::Sheet,
                false => Shown::Anchored,
            }}
            face={create_memo(move || face)}
            size={create_memo(move || size.get())}
            testing
            on_state
            on_dismiss={move || editor.close_popped(serial)}
        />
    }
}
