use crate::accessibility as wire;

impl From<accesskit::ActionRequest> for wire::ActionRequest {
    fn from(request: accesskit::ActionRequest) -> Self {
        Self {
            action: action_to(request.action),
            target_tree: request.target_tree.0.into_bytes(),
            target_node: request.target_node.0,
            data: request.data.map(data_to),
        }
    }
}

impl From<wire::ActionRequest> for accesskit::ActionRequest {
    fn from(request: wire::ActionRequest) -> Self {
        Self {
            action: action_from(request.action),
            target_tree: accesskit::TreeId(uuid::Uuid::from_bytes(request.target_tree)),
            target_node: accesskit::NodeId(request.target_node),
            data: request.data.map(data_from),
        }
    }
}

macro_rules! actions {
    ($($name:ident),* $(,)?) => {
        fn action_to(action: accesskit::Action) -> wire::Action {
            match action {
                $(accesskit::Action::$name => wire::Action::$name,)*
            }
        }

        fn action_from(action: wire::Action) -> accesskit::Action {
            match action {
                $(wire::Action::$name => accesskit::Action::$name,)*
            }
        }
    };
}

actions!(
    Click,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    HideTooltip,
    ShowTooltip,
    ReplaceSelectedText,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint,
    SetScrollOffset,
    SetTextSelection,
    SetSequentialFocusNavigationStartingPoint,
    SetValue,
    ShowContextMenu,
);

fn position_to(position: accesskit::TextPosition) -> wire::TextPosition {
    wire::TextPosition {
        node: position.node.0,
        character_index: position.character_index as u64,
    }
}

fn position_from(position: wire::TextPosition) -> accesskit::TextPosition {
    accesskit::TextPosition {
        node: accesskit::NodeId(position.node),
        character_index: position.character_index as usize,
    }
}

fn data_to(data: accesskit::ActionData) -> wire::ActionData {
    use accesskit::{ActionData, ScrollHint, ScrollUnit};
    let point = |point: accesskit::Point| wire::Point {
        x: point.x,
        y: point.y,
    };
    match data {
        ActionData::CustomAction(id) => wire::ActionData::CustomAction(id),
        ActionData::Value(value) => wire::ActionData::Value(value.into()),
        ActionData::NumericValue(value) => wire::ActionData::NumericValue(value),
        ActionData::ScrollUnit(unit) => wire::ActionData::ScrollUnit(match unit {
            ScrollUnit::Item => wire::ScrollUnit::Item,
            ScrollUnit::Page => wire::ScrollUnit::Page,
        }),
        ActionData::ScrollHint(hint) => wire::ActionData::ScrollHint(match hint {
            ScrollHint::TopLeft => wire::ScrollHint::TopLeft,
            ScrollHint::BottomRight => wire::ScrollHint::BottomRight,
            ScrollHint::TopEdge => wire::ScrollHint::TopEdge,
            ScrollHint::BottomEdge => wire::ScrollHint::BottomEdge,
            ScrollHint::LeftEdge => wire::ScrollHint::LeftEdge,
            ScrollHint::RightEdge => wire::ScrollHint::RightEdge,
        }),
        ActionData::ScrollToPoint(target) => wire::ActionData::ScrollToPoint(point(target)),
        ActionData::SetScrollOffset(offset) => wire::ActionData::SetScrollOffset(point(offset)),
        ActionData::SetTextSelection(selection) => {
            wire::ActionData::SetTextSelection(wire::TextSelection {
                anchor: position_to(selection.anchor),
                focus: position_to(selection.focus),
            })
        }
    }
}

fn data_from(data: wire::ActionData) -> accesskit::ActionData {
    use accesskit::{ActionData, ScrollHint, ScrollUnit};
    let point = |point: wire::Point| accesskit::Point::new(point.x, point.y);
    match data {
        wire::ActionData::CustomAction(id) => ActionData::CustomAction(id),
        wire::ActionData::Value(value) => ActionData::Value(value.into()),
        wire::ActionData::NumericValue(value) => ActionData::NumericValue(value),
        wire::ActionData::ScrollUnit(unit) => ActionData::ScrollUnit(match unit {
            wire::ScrollUnit::Item => ScrollUnit::Item,
            wire::ScrollUnit::Page => ScrollUnit::Page,
        }),
        wire::ActionData::ScrollHint(hint) => ActionData::ScrollHint(match hint {
            wire::ScrollHint::TopLeft => ScrollHint::TopLeft,
            wire::ScrollHint::BottomRight => ScrollHint::BottomRight,
            wire::ScrollHint::TopEdge => ScrollHint::TopEdge,
            wire::ScrollHint::BottomEdge => ScrollHint::BottomEdge,
            wire::ScrollHint::LeftEdge => ScrollHint::LeftEdge,
            wire::ScrollHint::RightEdge => ScrollHint::RightEdge,
        }),
        wire::ActionData::ScrollToPoint(target) => ActionData::ScrollToPoint(point(target)),
        wire::ActionData::SetScrollOffset(offset) => ActionData::SetScrollOffset(point(offset)),
        wire::ActionData::SetTextSelection(selection) => {
            ActionData::SetTextSelection(accesskit::TextSelection {
                anchor: position_from(selection.anchor),
                focus: position_from(selection.focus),
            })
        }
    }
}
