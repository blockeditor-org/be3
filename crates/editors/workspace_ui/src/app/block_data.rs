use std::collections::HashSet;
use std::rc::Rc;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Frame, Func, ItemSize, List, ReadSignal, Show, clone, component, create_memo, create_signal,
    view,
};
use block_editor_beui::beui::styled::{Caption, Code, Scroll, Tree, TreeRowFace};
use block_editor_beui::beui::unstyled::TreeItem;
use serde_json::Value;

use super::panel::Info;
use super::workspace::Workspace;

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;

#[derive(Clone, PartialEq)]
struct Row {
    path: String,
    label: String,
    depth: usize,
    expandable: bool,
    expanded: bool,
}

#[component]
pub(crate) fn BlockData(workspace: Rc<Workspace>, info: ReadSignal<Option<Info>>) -> NodeId {
    let (data, set_data) = create_signal(None::<String>);
    let reading = Rc::downgrade(&workspace);
    let read_info = info.clone();
    workspace.editor().each_frame(move || {
        let Some(workspace) = reading.upgrade() else {
            return;
        };
        let Some(id) = read_info.with(|info| info.as_ref().map(|info| info.item.id)) else {
            set_data.set(None);
            return;
        };
        set_data.set(workspace.debug_data(id));
    });
    let (expanded, set_expanded) = create_signal(HashSet::<String>::new());
    let parsed = create_memo(clone!(data -> move || {
        data.with(|data| data.as_ref().and_then(|data| serde_json::from_str::<Value>(data).ok()))
    }));
    let rows = create_memo(clone!(parsed expanded -> move || {
        parsed.with(|parsed| {
            parsed
                .as_ref()
                .map(|parsed| expanded.with(|expanded| build(parsed, expanded)))
                .unwrap_or_default()
        })
    }));
    let keys = create_memo(clone!(rows -> move || {
        rows.with(|rows| rows.iter().map(|row| row.path.clone()).collect::<Vec<_>>())
    }));
    let waiting = create_memo(clone!(data -> move || data.with(Option::is_none)));
    let raw = create_memo(clone!(data parsed -> move || {
        match parsed.with(Option::is_some) {
            true => String::new(),
            false => data.with(|data| data.clone().unwrap_or_default()),
        }
    }));
    let unparsed = create_memo(clone!(raw -> move || !raw.with(String::is_empty)));
    let item_rows = rows.clone();
    let item = Func::new(move |path: String| {
        item_rows.with(|rows| {
            rows.iter()
                .find(|row| row.path == path)
                .map_or_else(TreeItem::default, |row| TreeItem {
                    label: row.label.clone(),
                    depth: row.depth,
                    expandable: row.expandable,
                    expanded: row.expanded,
                    marked: false,
                })
        })
    });
    let content_rows = rows.clone();
    view! {
        <Frame padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=ROW_SPACING>
                <Show condition={waiting}>
                    <Caption content="This block has not finished loading yet." />
                </Show>
                <Show condition={unparsed}>
                    <Scroll @sizing=ItemSize::Percent(100.0)>
                        <Code content={raw} />
                    </Scroll>
                </Show>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Tree
                        keys={keys}
                        item={item}
                        selected={None}
                        spacing=ROW_SPACING
                        expand_on_select=false
                        on_select={move |_: String| {}}
                        on_expand={move |(path, expanded): (String, bool)| {
                            let _ = expanded;
                            set_expanded.update(|open| {
                                if !open.insert(path.clone()) {
                                    open.remove(&path);
                                }
                            });
                        }}
                    >
                        {move |face: TreeRowFace<String>| {
                            let path = face.key;
                            let label = create_memo(clone!(content_rows -> move || {
                                content_rows.with(|rows| {
                                    rows.iter()
                                        .find(|row| row.path == path)
                                        .map_or_else(String::new, |row| row.label.clone())
                                })
                            }));
                            view! {
                                <Code content={label} />
                            }
                        }}
                    </Tree>
                </Scroll>
            </List>
        </Frame>
    }
}

fn build(root: &Value, expanded: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    match root {
        Value::Object(fields) if !fields.is_empty() => {
            for (key, value) in fields {
                push(&mut rows, expanded, key.clone(), key, value, 0);
            }
        }
        root => push(&mut rows, expanded, "root".to_owned(), "root", root, 0),
    }
    rows
}

fn push(
    rows: &mut Vec<Row>,
    expanded: &HashSet<String>,
    path: String,
    key: &str,
    value: &Value,
    depth: usize,
) {
    let children: Vec<(String, &Value)> = match value {
        Value::Object(fields) if !fields.is_empty() => fields
            .iter()
            .map(|(key, value)| (key.clone(), value))
            .collect(),
        Value::Array(items) if !items.is_empty() => items
            .iter()
            .enumerate()
            .map(|(index, item)| (index.to_string(), item))
            .collect(),
        _ => Vec::new(),
    };
    let expandable = !children.is_empty();
    let open = expandable && expanded.contains(&path) != (depth == 0);
    rows.push(Row {
        label: format!("{key} {}", summary(value)),
        path: path.clone(),
        depth,
        expandable,
        expanded: open,
    });
    if !open {
        return;
    }
    for (child_key, child) in children {
        let child_path = format!("{path}.{child_key}");
        push(rows, expanded, child_path, &child_key, child, depth + 1);
    }
}

fn summary(value: &Value) -> String {
    match value {
        Value::Object(fields) if !fields.is_empty() => format!("{{{}}}", fields.len()),
        Value::Array(items) if !items.is_empty() => format!("[{}]", items.len()),
        Value::Object(_) => "{}".to_owned(),
        Value::Array(_) => "[]".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
    }
}
