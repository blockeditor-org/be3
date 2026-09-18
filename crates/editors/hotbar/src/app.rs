use block_client::block_ref::BlockRef;
use block_client::blocks::hotbar::HotbarSlot;
use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

mod ui;

use ui::HotbarView;

pub struct HotbarApp;

impl block_editor_plugin::BeuiApp for HotbarApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <HotbarView editor={editor} />
        }
    }
}

pub fn without_component(slots: &[HotbarSlot], compiled: BlockRef) -> Vec<HotbarSlot> {
    slots
        .iter()
        .filter(|slot| !matches!(slot, HotbarSlot::Component { compiled: pinned, .. } if *pinned == compiled))
        .map(|slot| match slot {
            HotbarSlot::Folder { name, slots } => HotbarSlot::Folder {
                name: name.clone(),
                slots: without_component(slots, compiled),
            },
            other => other.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
