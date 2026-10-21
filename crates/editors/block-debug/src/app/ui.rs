use std::collections::HashSet;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::TreeItem;
use block_editor_beui::{BlockQuery, Editor};

use super::entries::{self, Row};

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;

#[component]
pub fn BlockDebug(editor: Editor) -> NodeId {
    let raw = editor.raw_block_content();
    let listed = editor.watch_blocks(BlockQuery::Block(editor.block_id()));
    editor.block_types_known();
    let roots = create_memo(move || {
        let types = editor.block_types();
        let info = listed.with(|listed| listed.as_ref().ok().and_then(|list| list.first().cloned()));
        vec![
            entries::block(info.as_ref(), types.as_ref()),
            raw.with(|raw| entries::content(raw.as_ref(), types.as_ref())),
        ]
    });
    let toggled = create_signal(HashSet::<String>::new());
    let rows = create_memo(move || {
        roots.with(|roots| toggled.with(|toggled| entries::rows(roots, toggled)))
    });
    let keys = create_memo(move || {
        rows.with(|rows| rows.iter().map(|row| row.path.clone()).collect::<Vec<_>>())
    });
    let item = Func::new(move |path: String| {
        rows.with(|rows| {
            rows.iter()
                .find(|row| row.path == path)
                .map_or_else(TreeItem::default, |row: &Row| TreeItem {
                    label: row.label.clone(),
                    depth: row.depth,
                    expandable: row.expandable,
                    expanded: row.expanded,
                })
        })
    });
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=0.0>
                <Tree
                    @sizing=ItemSize::Percent(100.0)
                    keys={keys}
                    item={item}
                    selected={None}
                    spacing=ROW_SPACING
                    padding=PADDING
                    row_test_id={Func::new(|path: String| format!("block-debug.{path}"))}
                    on_select={move |_: String| {}}
                    on_expand={move |(path, _): (String, bool)| {
                        toggled.update(|toggled| {
                            if !toggled.insert(path.clone()) {
                                toggled.remove(&path);
                            }
                        });
                    }}
                >|face: TreeRowFace<String>| {
                    let path = face.key;
                    let label = create_memo(move || {
                        rows.with(|rows| {
                            rows.iter()
                                .find(|row| row.path == path)
                                .map_or_else(String::new, |row| row.label.clone())
                        })
                    });
                    view! {
                        <Text style=TextStyle::CODE string={label} />
                    }
                }</Tree>
            </List>
        </Frame>
    }
}
