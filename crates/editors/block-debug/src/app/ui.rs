use std::collections::HashSet;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_TRIANGLE_ALERT;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::TreeItem;
use block_editor_beui::{BlockQuery, Editor, RawContent};

use super::entries::{self, Row};

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;
const SPACING: f32 = 8.0;

#[component]
pub fn BlockDebug(editor: Editor) -> NodeId {
    let raw = editor.raw_block_content();
    let listed = editor.watch_blocks(BlockQuery::Block(editor.block_id()));
    editor.block_types_known();
    let roots = create_memo(move || {
        let types = editor.block_types();
        let info =
            listed.with(|listed| listed.as_ref().ok().and_then(|list| list.first().cloned()));
        vec![
            entries::block(info.as_ref(), types.as_ref()),
            raw.with(|raw| entries::content(raw.as_ref(), types.as_ref())),
        ]
    });
    let toggled = create_signal(HashSet::<String>::new());
    let editable = editor.editable();
    let unlocked = create_signal(false);
    let warned = create_memo(move || editable.get() && !unlocked.get());
    let hinted = create_memo(move || editable.get() && unlocked.get());
    let editing = create_signal(None::<String>);
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
                    label: match editing.with(|editing| editing.as_ref() == Some(&path)) {
                        true => row.value.clone(),
                        false => row.label.clone(),
                    },
                    depth: row.depth,
                    expandable: row.expandable,
                    expanded: row.expanded,
                })
        })
    });
    let start = move |path: String| {
        let can = editable.get_untracked()
            && unlocked.get_untracked()
            && rows
                .with_untracked(|rows| rows.iter().any(|row| row.path == path && row.at.is_some()));
        if can {
            editing.set(Some(path));
        }
    };
    let commit = move |(path, typed): (String, String)| {
        editing.set(None);
        let at = rows.with_untracked(|rows| {
            rows.iter()
                .find(|row| row.path == path)
                .and_then(|row| row.at.clone())
        });
        let replaced = at.and_then(|at| {
            raw.with_untracked(|raw| {
                let raw = raw.as_ref()?;
                let bytes = entries::replaced(&raw.bytes, &at, &typed)?;
                Some(RawContent {
                    content_type: raw.content_type,
                    bytes,
                })
            })
        });
        if let Some(content) = replaced {
            editor.replace_raw_content(content);
        }
    };
    view! {
        <List spacing=0.0>
            <Show condition={warned}>
                <Frame padding=PADDING>
                    <Frame
                        fill=ColorRole::SurfaceRaised
                        border={Border::new(ColorRole::Warning)}
                        radius=Radius::Md
                        padding=PADDING
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                            <Text color=TextColor::Warning>
                                <Icon glyph={ICON_TRIANGLE_ALERT.to_owned()} />
                            </Text>
                            <Text @sizing=ItemSize::Percent(100.0) size=TextSize::Sm wrap=true>
                                "Changing this data by hand can corrupt the block, so that its editors can no longer open it."
                            </Text>
                            <Button
                                @test_id="block-debug.unlock"
                                label="Edit anyway"
                                variant=ButtonVariant::Secondary
                                on_click={move || unlocked.set(true)}
                            />
                        </List>
                    </Frame>
                </Frame>
            </Show>
            <Show condition={hinted}>
                <Frame padding={Sides::new(PADDING, PADDING, PADDING, 0.0)}>
                    <Text style=TextStyle::CAPTION color=TextColor::Muted wrap=true>
                        "Double-click a value, or press F2 on it, to change it."
                    </Text>
                </Frame>
            </Show>
            <Tree
                @sizing=ItemSize::Percent(100.0)
                keys={keys}
                item={item}
                selected={None}
                editing={editing}
                spacing=ROW_SPACING
                padding=PADDING
                row_test_id={Func::new(|path: String| format!("block-debug.{path}"))}
                on_select={move |_: String| {}}
                on_open={start}
                on_edit={start}
                on_rename={commit}
                on_cancel_edit={move |_: String| editing.set(None)}
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
        </List>
    }
}
