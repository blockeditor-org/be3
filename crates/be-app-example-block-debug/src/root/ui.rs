use std::collections::HashSet;

use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{BlockQuery, ChildBlock, ChildLayer, ChildMode, ChildTarget, Editor};

use super::entries::{self, Row};

const PADDING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs2;

#[component]
pub fn BlockDebug(editor: Editor) -> NodeId {
    let head = editor.stored_head(editor.block_id());
    let listed = editor.watch_blocks(BlockQuery::Block(editor.block_id()));
    editor.block_types_known();
    let roots = create_memo(move || {
        let types = editor.block_types();
        let info =
            listed.with(|listed| listed.as_ref().ok().and_then(|list| list.first().cloned()));
        vec![
            entries::block(info.as_ref(), types.as_ref()),
            head.with(|head| entries::content(head, types.as_ref())),
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
    let target = create_memo(move || {
        listed.with(|listed| {
            listed
                .as_ref()
                .ok()
                .and_then(|list| list.first())
                .map(|info| ChildTarget::new(info.id, info.block_type))
        })
    });
    view! {
        <List spacing=Space::None>
            <Frame padding={Padding::symmetric(PADDING, Space::Xs)}>
                <Preview editor target />
            </Frame>
            <ListItem grow=1.0>
                <Tree
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
                        <Text style=TextStyle::CODE>{label}</Text>
                    }
                }</Tree>
            </ListItem>
        </List>
    }
}

/// The block in its own editor, in a popover: a child placed above an anchored view, which
/// every host shows over it (guides/running/web.md, Apps on the page).
#[component]
fn Preview(editor: Editor, target: Memo<Option<ChildTarget>>) -> NodeId {
    view! {
        <Popover label="Preview" @test_id={"block-debug.preview"}>|_|
            <Frame width={Length::Size(PREVIEW.0)} height={Length::Size(PREVIEW.1)}>
                <ChildBlock
                    editor
                    block={target}
                    mode=ChildMode::Passive
                    layer=ChildLayer::Above
                    @test_id={"block-debug.preview.child"}
                />
            </Frame>
        </Popover>
    }
}

const PREVIEW: (Size, Size) = (Size::Custom(280.0), Size::Custom(180.0));
