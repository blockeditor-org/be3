use beui::Overflow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use workspace_ui_types::ViewState;

use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_ARROW_UP, ICON_PLUS};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::block_ui::Press;

use super::rows::{Row, RowKey, Tree as FileTree};

const PADDING: f32 = 8.0;
const ZOOMED: &str = "zoomed";

#[derive(Clone, PartialEq)]
pub(crate) struct Focus {
    at: usize,
    depth: usize,
    pub(crate) row: Row,
    pub(crate) path: String,
}

#[derive(Clone, Copy)]
pub(crate) struct Zoom {
    tree: FileTree,
    zoomed: Signal<Option<RowKey>>,
    focus: Memo<Option<Focus>>,
}

pub(crate) fn zooms(row: &Row) -> bool {
    match row.id {
        None => row.section,
        Some(_) => !row.is_reference && (row.is_workspace() || row.press == Press::Zoom),
    }
}

impl Zoom {
    pub(crate) fn new(editor: Editor, tree: FileTree) -> Self {
        let zoomed = create_signal(None::<RowKey>);
        let rows = tree.rows();
        let focus = create_memo(move || {
            let key = zoomed.get()?;
            rows.with(|rows| {
                let at = rows.iter().position(|row| row.key == key)?;
                let depth = rows[at].depth;
                let mut above = Vec::new();
                let mut reach = depth;
                for row in rows[..at].iter().rev() {
                    if row.depth < reach {
                        above.push(row.label.clone());
                        reach = row.depth;
                    }
                }
                let path = std::iter::once("Files".to_owned())
                    .chain(above.into_iter().rev())
                    .collect::<Vec<_>>()
                    .join(" / ");
                Some(Focus {
                    at,
                    depth,
                    row: rows[at].clone(),
                    path,
                })
            })
        });
        let seen = RefCell::new(None::<RowKey>);
        create_effect(move || {
            let Some(key) = zoomed.get() else {
                return;
            };
            if focus.with(Option::is_some) {
                *seen.borrow_mut() = Some(key);
            } else if seen.borrow().as_ref() == Some(&key) {
                zoomed.set(None);
            }
        });
        let restored = Rc::new(Cell::new(false));
        let adopted = Rc::clone(&restored);
        create_binding(move || {
            if adopted.get() {
                return;
            }
            let Some(view) = editor.view_content() else {
                return;
            };
            let Ok(saved) = view.read(|held| {
                held.root()
                    .state(ZOOMED)
                    .and_then(ViewState::value::<Option<RowKey>>)
            }) else {
                return;
            };
            adopted.set(true);
            untrack(|| zoomed.set(saved.flatten()));
        });
        create_effect(move || {
            let key = zoomed.get();
            if !restored.get() {
                return;
            }
            let state = ViewState::new(&key, Vec::new());
            untrack(|| editor.set_view_state(ZOOMED, Some(&state)));
        });
        Self {
            tree,
            zoomed,
            focus,
        }
    }

    pub(crate) fn focus(&self) -> Memo<Option<Focus>> {
        self.focus
    }

    pub(crate) fn zoomed(&self) -> ReadSignal<Option<RowKey>> {
        self.zoomed.read_only()
    }

    pub(crate) fn keys(&self) -> Memo<Vec<RowKey>> {
        let (rows, focus) = (self.tree.rows(), self.focus);
        create_memo(move || {
            let focus = focus.get();
            rows.with(|rows| {
                let shown = match &focus {
                    None => rows,
                    Some(focus) => {
                        let below = &rows[focus.at + 1..];
                        let inside = below
                            .iter()
                            .take_while(|row| row.depth > focus.depth)
                            .count();
                        &below[..inside]
                    }
                };
                shown.iter().map(|row| row.key.clone()).collect()
            })
        })
    }

    pub(crate) fn depth(&self, depth: usize) -> usize {
        self.focus.with(|focus| match focus {
            Some(focus) => depth.saturating_sub(focus.depth + 1),
            None => depth,
        })
    }

    pub(crate) fn enter(&self, key: RowKey) {
        let rows = self.tree.rows();
        let Some(row) = rows.with_untracked(|rows| rows.iter().find(|row| row.key == key).cloned())
        else {
            return;
        };
        if row.expandable && !row.expanded {
            self.tree.toggle(&row.key, row.id, false);
        }
        self.zoomed.set(Some(key));
    }

    pub(crate) fn up(&self) {
        let rows = self.tree.rows();
        let Some(focus) = self.focus.get_untracked() else {
            return;
        };
        let parent = rows.with_untracked(|rows| {
            rows[..focus.at]
                .iter()
                .rev()
                .find(|row| row.depth < focus.depth)
                .map(|row| row.key.clone())
        });
        self.zoomed.set(parent);
    }

    pub(crate) fn leave_for(&self, key: &RowKey) {
        let inside = match (self.zoomed.get_untracked(), key) {
            (None, _) => true,
            (Some(RowKey::Block(zoomed)), RowKey::Block(path)) => path.starts_with(&zoomed),
            (Some(_), _) => false,
        };
        if !inside {
            self.zoomed.set(None);
        }
    }
}

#[component]
pub(crate) fn ZoomBar(
    zoom: Zoom,
    addable: Memo<bool>,
    new_button: NodeRef,
    on_new: ClickCallback,
) -> NodeId {
    let focus = zoom.focus();
    let path = create_memo(move || focus.get().map(|focus| focus.path).unwrap_or_default());
    let name = create_memo(move || focus.get().map(|focus| focus.row.label).unwrap_or_default());
    view! {
        <Frame fill=ColorRole::Background padding=PADDING>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Button
                    icon_only=true
                    @test_id={"file-tree.back"}
                    glyph=ICON_ARROW_UP
                    label="Up one level"
                    on_click={move || zoom.up()}
                />
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                    <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>{path}</Text>
                    <Text style=TextStyle::HEADING overflow=Overflow::Ellipsis>{name}</Text>
                </List>
                <Show condition={addable}>
                    <Button
                        icon_only=true
                        @node_ref={&new_button}
                        @test_id={"file-tree.new"}
                        glyph=ICON_PLUS
                        label="New"
                        on_click={on_new}
                    />
                </Show>
            </List>
        </Frame>
    }
}
