use std::cell::RefCell;
use std::rc::Rc;

use beui::dock::styled::{
    CARD_GAP, CARD_INSET, CARD_MARGIN, CARD_POINTER, DockCard, PointerSide, Pointing,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Pos2, Rect, Vec2, WindowPlacement, WindowReport, WindowRole};
use block_editor_plugin::{BlockFilter, BlockParent, BlockPick, Failure, PickedBlock};

use crate::{ChildLayer, ChildMode, ChildState, Editor, Subregion, SubregionContent, node_anchor};

const UNSIZED: Vec2 = Vec2::new(256.0, 320.0);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PickAnchor {
    #[default]
    Centre,
    Rect(Rect),
    Node(NodeRef),
}

impl From<Rect> for PickAnchor {
    fn from(rect: Rect) -> Self {
        Self::Rect(rect)
    }
}

impl From<Option<Rect>> for PickAnchor {
    fn from(rect: Option<Rect>) -> Self {
        rect.map_or(Self::Centre, Self::Rect)
    }
}

impl From<NodeRef> for PickAnchor {
    fn from(node: NodeRef) -> Self {
        Self::Node(node)
    }
}

impl From<&NodeRef> for PickAnchor {
    fn from(node: &NodeRef) -> Self {
        Self::Node(*node)
    }
}

impl PickAnchor {
    fn rect(self) -> Option<Rect> {
        match self {
            Self::Centre => None,
            Self::Rect(rect) => Some(rect),
            Self::Node(node) => node_anchor(&node),
        }
    }
}

type Answer = Box<dyn FnOnce(Result<PickedBlock, Failure>)>;

#[derive(Clone)]
pub(crate) struct Picking(Rc<Asked>);

struct Asked {
    serial: u64,
    filter: BlockFilter,
    parent: BlockParent,
    anchor: PickAnchor,
    touch: bool,
    answer: RefCell<Option<Answer>>,
}

impl PartialEq for Picking {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Picking {
    pub(crate) fn new(
        serial: u64,
        filter: BlockFilter,
        parent: BlockParent,
        anchor: PickAnchor,
        touch: bool,
        answer: Answer,
    ) -> Self {
        Self(Rc::new(Asked {
            serial,
            filter,
            parent,
            anchor,
            touch,
            answer: RefCell::new(Some(answer)),
        }))
    }

    pub(crate) fn serial(&self) -> u64 {
        self.0.serial
    }

    pub(crate) fn answer(&self, answer: BlockPick) {
        let Some(answer_with) = self.0.answer.borrow_mut().take() else {
            return;
        };
        match answer {
            Ok(Some(block)) => answer_with(Ok(block)),
            Ok(None) => {}
            Err(failure) => answer_with(Err(failure)),
        }
    }
}

#[component]
pub(crate) fn Pickers(editor: Editor) -> NodeId {
    let picking = editor.picking();
    let keys = create_memo(move || {
        picking
            .with(|picking| picking.as_ref().map(Picking::serial))
            .into_iter()
            .collect::<Vec<_>>()
    });
    view! {
        <Layers>
            <ForEach keys>|serial: u64| {
                match picking.get_untracked().filter(|picking| picking.serial() == serial) {
                    Some(picking) => view! {
                        <PickerPopover editor picking />
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
fn PickerPopover(editor: Editor, picking: Picking) -> NodeId {
    let sheet = picking.0.touch || crate::phone_layout().get_untracked();
    let serial = picking.serial();
    let content = SubregionContent::Picker {
        filter: picking.0.filter.clone(),
        parent: picking.0.parent,
        touch: sheet,
    };
    let size = create_signal(None::<Vec2>);
    let card = create_signal(false);
    let child = move || {
        let content = content.clone();
        view! {
            <Subregion
                editor
                placed={Some(content)}
                mode=ChildMode::Live
                layer=ChildLayer::Above
                punch=false
                @test_id={"picker.child"}
                on_state={move |state: ChildState| {
                    let wanted = state.intrinsic_size.filter(|size| size.x > 0.0 && size.y > 0.0);
                    if size.get_untracked() != wanted {
                        size.set(wanted);
                    }
                    let carded = state.card && !sheet;
                    if card.get_untracked() != carded {
                        card.set(carded);
                    }
                    if let Some(error) = state.error.clone() {
                        return editor.finish_pick(serial, Err(Failure::Failed(error)));
                    }
                    if let Some(answer) = state.picked {
                        editor.finish_pick(serial, answer);
                    }
                }}
            />
        }
    };
    let anchor = picking.0.anchor.rect();
    let own = use_window();
    let viewport = use_viewport_size();
    let content_size = create_memo(move || size.get().unwrap_or(UNSIZED));
    let hung = create_memo(move || match (sheet, card.get()) {
        (true, _) => Hung {
            window: Rect::from_min_size(Pos2::ZERO, viewport.get()),
            pointing: None,
        },
        (false, true) => card_hung(anchor, content_size.get(), viewport.get()),
        (false, false) => Hung {
            window: Rect::from_min_size(
                hang(anchor, content_size.get(), viewport.get()),
                content_size.get(),
            ),
            pointing: None,
        },
    });
    let role = create_memo(move || match sheet || card.get() {
        true => WindowRole::Normal,
        false => WindowRole::Popup,
    });
    let shown = create_memo(move || hung.get().window.size());
    let at = create_memo(move || hung.get().window.min);
    let window = match picking.0.anchor {
        PickAnchor::Node(node) => node.try_get().map(window_of),
        _ => None,
    }
    .or(own.map(|handle| handle.id));
    let placement = create_memo(move || match window {
        Some(window) => WindowPlacement::Over {
            window,
            at: at.get(),
        },
        None => WindowPlacement::AtPointer,
    });
    view! {
        <Window
            id={None}
            size={shown}
            at
            placement
            role
            on_report={move |report: WindowReport| {
                if matches!(report, WindowReport::Closed(_))
                    && role.get_untracked() == WindowRole::Popup
                {
                    editor.finish_pick(serial, Ok(None));
                }
            }}
        >|handle: WindowHandle| {
            let node = child();
            let shown_size = create_memo(move || match card.get() {
                true => content_size.get(),
                false => handle.size.get(),
            });
            let card_width = create_memo(move || Size::Custom(shown_size.get().x));
            let card_height = create_memo(move || Size::Custom(shown_size.get().y));
            let carded = create_memo(move || card.get());
            let inset = create_memo(move || match card.get() {
                true => Padding::all(Space::Custom(CARD_INSET)),
                false => Padding::ZERO,
            });
            let pointing = create_memo(move || hung.get().pointing);
            let pointer = NodeRef::new();
            let outside = move |_: Pos2| {
                if !sheet {
                    editor.finish_pick(serial, Ok(None));
                }
            };
            view! {
                <Root clear_color=ColorRole::None>
                    <Interactive on_press_outside={outside}>
                        <Layers>
                            <Show condition={carded}>
                                <DockCard pointing pointer>
                                    <Frame width=card_width height=card_height />
                                </DockCard>
                            </Show>
                            <Frame padding={inset}>
                                <Frame width=card_width height=card_height>{node}</Frame>
                            </Frame>
                        </Layers>
                    </Interactive>
                </Root>
            }
            .child()
        }</Window>
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Hung {
    window: Rect,
    pointing: Option<Pointing>,
}

fn card_hung(anchor: Option<Rect>, content: Vec2, viewport: Vec2) -> Hung {
    let margin = f32::from(CARD_MARGIN);
    let face = content + Vec2::splat(2.0 * CARD_INSET);
    let Some(anchor) = anchor else {
        return Hung {
            window: Rect::from_min_size(hang(None, face, viewport), face),
            pointing: None,
        };
    };
    let reach = f32::from(CARD_GAP) - CARD_POINTER;
    let x = (anchor.center().x - face.x / 2.0)
        .min(viewport.x - face.x - margin)
        .max(margin.min(viewport.x - face.x).max(0.0));
    let below = anchor.max.y + reach + face.y <= viewport.y - margin;
    let above = anchor.min.y - reach - face.y >= margin;
    let (y, side) = match (below, above) {
        (false, true) => (anchor.min.y - reach - face.y, PointerSide::Bottom),
        _ => (anchor.max.y + reach, PointerSide::Top),
    };
    Hung {
        window: Rect::from_min_size(Pos2::new(x, y), face),
        pointing: Some(Pointing {
            side,
            at: anchor.center().x - x - CARD_POINTER,
        }),
    }
}

fn hang(anchor: Option<Rect>, size: Vec2, viewport: Vec2) -> Pos2 {
    let Some(anchor) = anchor else {
        return Pos2::new(
            ((viewport.x - size.x) / 2.0).max(0.0),
            ((viewport.y - size.y) / 2.0).max(0.0),
        );
    };
    let below = anchor.max.y + size.y <= viewport.y;
    let above = anchor.min.y - size.y >= 0.0;
    let y = match (below, above) {
        (false, true) => anchor.min.y - size.y,
        _ => anchor.max.y,
    };
    Pos2::new(anchor.min.x.max(0.0), y)
}
