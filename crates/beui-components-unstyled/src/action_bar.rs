use beui_macros::component;
use beui_view::actions::{Action, MenuNode, Pin, tidy_menu};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

/// How a bar shows the actions pinned with `Pin::Label`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionLabels {
    /// With their labels.
    #[default]
    Shown,
    /// As their icons; one with no glyph is left to the overflow menu.
    Icons,
    /// Left to the overflow menu.
    Overflow,
}

impl ActionLabels {
    fn shows(self, action: &Action) -> bool {
        match self {
            ActionLabels::Shown => true,
            ActionLabels::Icons => action.glyph().is_some(),
            ActionLabels::Overflow => false,
        }
    }
}

pub struct ActionBarHandle {
    pub icons: Memo<Vec<Action>>,
    pub labels: Memo<Vec<Action>>,
    pub overflow: Memo<Vec<MenuNode>>,
}

#[component]
pub fn ActionBar(
    menu: Prop<Vec<MenuNode>>,
    #[prop(default = ActionLabels::Shown)] labels: Prop<ActionLabels>,
    #[prop(default = false)] title: Prop<bool>,
    #[prop(children)] content: Render<ActionBarHandle, NodeId>,
) -> NodeId {
    let menu = create_memo(move || menu.get());
    let shows = move |action: &Action| match action.pinned() {
        Some(Pin::Icon) => true,
        Some(Pin::Label) => labels.get().shows(action),
        Some(Pin::Title) => title.get(),
        None => false,
    };
    let pinned_as = move |wanted: Pin| {
        create_memo(move || {
            menu.with(|nodes| {
                pinned(nodes)
                    .into_iter()
                    .filter(|action| action.pinned() == Some(wanted) && shows(action))
                    .collect::<Vec<_>>()
            })
        })
    };
    let icons = pinned_as(Pin::Icon);
    let labelled = pinned_as(Pin::Label);
    let overflow = create_memo(move || unpinned(menu.get(), &shows));
    content.call(ActionBarHandle {
        icons,
        labels: labelled,
        overflow,
    })
}

pub fn title_action(nodes: &[MenuNode]) -> Option<Action> {
    pinned(nodes)
        .into_iter()
        .find(|action| action.pinned() == Some(Pin::Title))
}

fn pinned(nodes: &[MenuNode]) -> Vec<Action> {
    nodes
        .iter()
        .flat_map(|node| match node {
            MenuNode::Action { action, .. } if action.pinned().is_some() => vec![action.clone()],
            MenuNode::Submenu(submenu) => pinned(&submenu.items),
            MenuNode::Action { .. } | MenuNode::Separator => Vec::new(),
        })
        .collect()
}

fn unpinned(nodes: Vec<MenuNode>, shows: &impl Fn(&Action) -> bool) -> Vec<MenuNode> {
    let kept = nodes
        .into_iter()
        .filter_map(|node| match node {
            MenuNode::Action { action, .. } if shows(&action) => None,
            MenuNode::Submenu(mut submenu) => {
                submenu.items = unpinned(std::mem::take(&mut submenu.items), shows);
                Some(MenuNode::Submenu(submenu))
            }
            node => Some(node),
        })
        .collect();
    tidy_menu(kept)
}
