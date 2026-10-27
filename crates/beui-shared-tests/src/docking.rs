use beui::dock::styled::Docking;
use beui::dock::unstyled::{
    DockPane, DockSplit, DockState, DockTab, DockTree, DockTreeEntry, DockingLayout, TabId, Tree,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{Direction, ItemSize};

const ALPHA: TabId = TabId::new(1);
const BETA: TabId = TabId::new(2);
const GAMMA: TabId = TabId::new(3);

fn name(tab: TabId) -> &'static str {
    match tab {
        ALPHA => "Alpha",
        BETA => "Beta",
        GAMMA => "Gamma",
        _ => "?",
    }
}

fn describe(state: &DockState, tree: &DockTree) -> String {
    match tree {
        DockTree::Tabs {
            entries, active, ..
        } => {
            let tabs: Vec<String> = entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let mark = if index == *active { "*" } else { "" };
                    match entry {
                        DockTreeEntry::Tab(tab) => format!("{}{mark}", name(*tab)),
                        DockTreeEntry::Group(group) => {
                            format!("{{{}}}{mark}", describe(state, group))
                        }
                    }
                })
                .collect();
            format!("[{}]", tabs.join(" "))
        }
        DockTree::Split {
            direction,
            fraction,
            first,
            second,
        } => {
            let between = match direction {
                Direction::Horizontal => "|",
                Direction::Vertical => "/",
            };
            format!(
                "({} {between}{:.1} {})",
                describe(state, first),
                fraction,
                describe(state, second)
            )
        }
    }
}

fn report(state: &DockState) -> String {
    let windows = state.windows();
    state
        .surfaces()
        .into_iter()
        .filter_map(|surface| {
            let tree = state.tree(Tree::Surface(surface))?;
            let described = describe(state, &tree);
            Some(match state.window_rect(surface).filter(|_| windows.contains(&surface)) {
                Some(rect) => format!("window at {:.0},{:.0} {described}", rect.min.x, rect.min.y),
                None => described,
            })
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[component]
fn Panel(tab: TabId) -> NodeId {
    let id = format!("docking.{}", name(tab).to_lowercase());
    view! {
        <Frame @test_id={id} fill=ColorRole::Surface padding=Padding::all(Space::Md)>
            <Text>{name(tab)}</Text>
        </Frame>
    }
}

#[component]
pub fn DockingFixture() -> NodeId {
    let layout = DockingLayout::new();
    let open = create_signal(vec![ALPHA, BETA]);
    let state = layout.state();
    let described = create_memo(move || state.with(report));
    let close = move |tab: TabId| open.update(|open| open.retain(|held| *held != tab));
    view! {
        <List spacing=Space::Md>
            <Text>{described}</Text>
            <Frame width=Size::Custom(900.0) height=Size::Custom(520.0)>
                <Docking layout>
                    <DockSplit id="root" direction=Direction::Horizontal fraction=0.5>
                        <DockPane id="left">
                            <ForEach keys={open}>|tab: TabId|
                                <DockTab id=tab title={name(tab)} on_close={move || close(tab)}>
                                    <Panel tab />
                                </DockTab>
                            </ForEach>
                        </DockPane>
                        <DockPane id="right">
                            <DockTab id=GAMMA title="Gamma">
                                <Panel tab=GAMMA />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </Frame>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}
