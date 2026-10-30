use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Rect, Vec2};
use block_editor_plugin::{BlockFilter, BlockParent, BlockPick, Failure, PickedBlock};

use crate::popover::{ChildPopover, Face, Popovers, Shown};
use crate::{ChildState, Editor, SubregionContent};

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
            <Popovers editor />
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
    let size = create_signal(LAST_SIZE.with(Cell::get));
    let card = create_signal(false);
    let face = create_memo(move || match card.get() {
        true => Face::Card,
        false => Face::Bare,
    });
    let shown = create_memo(move || size.get());
    let on_state = Callback::new(move |state: ChildState| {
        let wanted = state
            .intrinsic_size
            .filter(|size| size.x > 0.0 && size.y > 0.0);
        if let Some(wanted) = wanted
            && size.get_untracked() != wanted
        {
            LAST_SIZE.with(|last| last.set(wanted));
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
    });
    view! {
        <ChildPopover
            editor
            content
            anchor={picking.0.anchor}
            shown={match sheet {
                true => Shown::Filled,
                false => Shown::Anchored,
            }}
            face
            size={shown}
            testing="picker.child"
            on_state
            on_dismiss={move || editor.finish_pick(serial, Ok(None))}
        />
    }
}

thread_local! {
    static LAST_SIZE: Cell<Vec2> = const { Cell::new(UNSIZED) };
}
