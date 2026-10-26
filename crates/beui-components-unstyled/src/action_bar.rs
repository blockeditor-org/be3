use beui_view::reactive::NodeId;
use beui_macros::component;
use beui_view::actions::{Action, MenuNode, Pin, tidy_menu};
use beui_view::reactive::*;

pub struct ActionBarHandle {
    pub icons: Memo<Vec<Action>>,
    pub labels: Memo<Vec<Action>>,
    pub overflow: Memo<Vec<MenuNode>>,
}

#[component]
pub fn ActionBar(
    menu: Prop<Vec<MenuNode>>,
    #[prop(default = true)] labels: Prop<bool>,
    #[prop(default = false)] title: Prop<bool>,
    #[prop(children)] content: Render<ActionBarHandle>,
) -> NodeId {
    let menu = create_memo(move || menu.get());
    let shows = move |pin: Pin| match pin {
        Pin::Icon => true,
        Pin::Label => labels.get(),
        Pin::Title => title.get(),
    };
    let pinned_as = move |wanted: Pin| {
        create_memo(move || {
            if !shows(wanted) {
                return Vec::new();
            }
            menu.with(|nodes| {
                pinned(nodes)
                    .into_iter()
                    .filter(|action| action.pinned() == Some(wanted))
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

fn unpinned(nodes: Vec<MenuNode>, shows: &impl Fn(Pin) -> bool) -> Vec<MenuNode> {
    let kept = nodes
        .into_iter()
        .filter_map(|node| match node {
            MenuNode::Action { action, .. } if action.pinned().is_some_and(shows) => None,
            MenuNode::Submenu(mut submenu) => {
                submenu.items = unpinned(std::mem::take(&mut submenu.items), shows);
                Some(MenuNode::Submenu(submenu))
            }
            node => Some(node),
        })
        .collect();
    tidy_menu(kept)
}
