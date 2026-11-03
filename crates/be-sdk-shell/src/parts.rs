use be_sdk_beui::ChildTarget;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::styled::TabId;
use be_sdk_beui::beui::reactive::RenderFn;

use super::workspace::Workspace;

pub trait TabParts {
    fn bar(&self) -> RenderFn<TabId, NodeId>;

    fn shown_instead(&self, workspace: Workspace, tab: TabId) -> Option<ChildTarget>;

    fn details(&self, workspace: Workspace, tab: TabId);
}
