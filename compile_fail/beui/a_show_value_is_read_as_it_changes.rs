use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(picked: Prop<Option<u32>>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Show value=true><Spacer /></Show> //~ ERROR this prop is read as it changes
            <Show value={picked.get_untracked().is_some()}><Spacer /></Show> //~ ERROR this prop is read as it changes
        </List>
    }
}
