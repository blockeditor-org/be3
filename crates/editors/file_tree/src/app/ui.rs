use std::collections::HashSet;
use std::rc::Rc;

use block::BlockParent;
use block_editor_plugin::beui::accesskit::{Node as AccessNode, Role};
use block_editor_plugin::beui::icons::{
    ICON_ADD, ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_AUTO_AWESOME, ICON_KEYBOARD_ARROW_DOWN,
    ICON_KEYBOARD_ARROW_RIGHT, ICON_MY_LOCATION,
};
use block_editor_plugin::beui::reactive::{
    Align, ClickCatcher, Direction, Frame, ItemSize, List, Memo, NodeRef, ReadSignal, Show, Spacer,
    clone, component, component_rect, create_memo, create_signal, view, with_document,
};
use block_editor_plugin::beui::styled::theme::{FONT_SMALL, RADIUS};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, ContextMenu, IconButton, IconSized, Scroll, Tooltip,
    use_theme,
};
use block_editor_plugin::beui::unstyled::{
    self, ButtonHandle, Edge, Floating, MenuItem, TreeItem, TreeRowHandle, tree_row_node,
};
use block_editor_plugin::beui::{Color32, CursorIcon, NodeId, PointerPress, Pos2};
use block_editor_plugin::{BlockFilter, BlockPicker, BlockSource, Editor, Toolbar};
use uuid::Uuid;

use super::rows::{Row, RowKey, Tree as FileTree, access_hint, access_marker};

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 6.0;
const ROW_HEIGHT: f32 = 22.0;
const ROW_INDENT: f32 = 12.0;
const CHEVRON_WIDTH: f32 = 16.0;
const ADD_WIDTH: f32 = 20.0;
const DRAG_THRESHOLD: f32 = 6.0;

#[component]
pub fn FileTreeEditor(editor: Editor) -> NodeId {
    let tree = FileTree::watch(&editor);
    let picker = picker(&editor, Rc::clone(&tree));
    let held: Held = Rc::new(std::cell::Cell::new(None));
    let (reveal, set_reveal) = create_signal(None::<RowKey>);
    let keys = tree.keys();
    let rows = tree.rows();
    let (focused, set_focused) = create_signal(None::<RowKey>);
    let watching = editor.host().clone();
    editor.each_frame(move || set_focused.set(focused_key(&watching)));

    let shown = create_memo(clone!(focused keys -> move || {
        let focused = focused.get()?;
        keys.with(|keys| deepest_shown(keys, &focused))
    }));
    let selected = create_memo(clone!(shown focused -> move || {
        shown.get().filter(|key| Some(key) == focused.get().as_ref())
    }));
    let buried = create_memo(clone!(shown selected -> move || {
        shown.get().filter(|_| selected.get().is_none())
    }));

    let (astray, set_astray) = create_signal(None::<Astray>);
    let tree_ref = NodeRef::new();
    let scroll_ref = NodeRef::new();
    let strayed = (
        shown.clone(),
        buried.clone(),
        tree_ref.clone(),
        scroll_ref.clone(),
    );
    editor.each_frame(move || {
        set_astray.set(stray(&strayed.0, &strayed.1, &strayed.2, &strayed.3));
    });
    let adrift = create_memo(clone!(astray -> move || astray.get().is_some()));
    let edge = create_memo(clone!(astray -> move || match astray.get() {
        Some(Astray::Above) => Edge::Top,
        _ => Edge::Bottom,
    }));
    let stray_glyph = create_memo(clone!(astray -> move || match astray.get() {
        Some(Astray::Above) => ICON_ARROW_UPWARD.to_owned(),
        _ => ICON_ARROW_DOWNWARD.to_owned(),
    }));
    let stray_label = create_memo(clone!(focused rows -> move || {
        let Some(RowKey::Block(path)) = focused.get() else {
            return "Reveal the block being shown".to_owned();
        };
        let Some(id) = path.last().copied() else {
            return "Reveal the block being shown".to_owned();
        };
        rows.with(|rows| {
            rows.iter()
                .find(|row| row.id == Some(id))
                .map_or_else(|| "Reveal the block being shown".to_owned(), |row| row.label.clone())
        })
    }));

    let item = clone!(rows buried -> move |key: RowKey| {
        let row = rows.with(|rows| rows.iter().find(|row| row.key == key).cloned());
        TreeItem {
            label: row.as_ref().map(|row| row.label.clone()).unwrap_or_default(),
            depth: row.as_ref().map_or(0, |row| row.depth),
            expandable: row.as_ref().is_some_and(|row| row.expandable),
            expanded: row.as_ref().is_some_and(|row| row.expanded),
            marked: buried.get().as_ref() == Some(&key),
        }
    });

    let open = clone!(tree rows editor -> move |key: RowKey| {
        let Some(row) = rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()) else {
            return;
        };
        let (Some(id), true) = (row.id, row.access.can_view()) else {
            return;
        };
        let _ = &tree;
        match row.container {
            Some(container) => editor.host().open_block_via(id, row.block_type, container),
            None => editor.host().open_block(id, row.block_type),
        }
    });
    let expand = clone!(tree -> move |(key, expanded): (RowKey, bool)| {
        let id = match &key {
            RowKey::Block(path) => path.last().copied(),
            RowKey::Orphans | RowKey::Note(_) => None,
        };
        tree.toggle(&key, id, !expanded);
    });

    let find = clone!(tree editor set_reveal -> move || {
        let host = editor.host();
        let focused = host.focused_block();
        if focused.block_id.is_none() {
            return;
        }
        tree.show_orphans();
        tree.expand(focused.via.iter().rev().copied());
        let Some(key) = focused_key(host) else {
            return;
        };
        set_reveal.set(Some(key));
    });
    let reveal_stray = find.clone();
    let add_root = clone!(picker editor -> move || {
        picker.open(
            &editor,
            None,
            [editor.block_id()].into_iter().collect::<HashSet<Uuid>>(),
        );
    });
    let failure = picker.error();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Toolbar shown={chrome}>
                    <IconButton
                        glyph={ICON_ADD.to_owned()}
                        label="Add a root block"
                        @test_id={"file-tree.add-root"}
                        on_click={add_root}
                    />
                    <IconButton
                        glyph={ICON_MY_LOCATION.to_owned()}
                        label="Reveal the block being shown"
                        @test_id={"file-tree.reveal"}
                        on_click={find}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </Toolbar>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <List spacing=ROW_SPACING>
                        <Show condition={failed}>
                            <Caption content={reason} color={theme.danger.clone()} />
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&scroll_ref}>
                            <unstyled::Tree
                                @node_ref={&tree_ref}
                                keys={keys}
                                item={item}
                                selected={selected}
                                reveal={reveal}
                                spacing=2.0
                                expand_on_select=false
                                on_select={open}
                                on_expand={expand}
                            >
                                {move |handle: TreeRowHandle<RowKey>| {
                                    let row = tree.row(handle.key.clone());
                                    view! {
                                        <TreeRow
                                            editor={editor.clone()}
                                            tree={Rc::clone(&tree)}
                                            picker={picker.clone()}
                                            held={Rc::clone(&held)}
                                            row={row}
                                            handle={handle}
                                        />
                                    }
                                }}
                            </unstyled::Tree>
                        </Scroll>
                        <Floating anchor={scroll_ref} edge={edge} open={adrift}>
                            <Button
                                glyph={stray_glyph}
                                label={stray_label}
                                variant=ButtonVariant::Secondary
                                @test_id={"file-tree.stray"}
                                on_click={reveal_stray}
                            />
                        </Floating>
                    </List>
                </Frame>
            </List>
        </Frame>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Astray {
    Above,
    Below,
}

fn focused_key(host: &block_editor_plugin::EditorHost) -> Option<RowKey> {
    let focused = host.focused_block();
    let id = focused.block_id?;
    let mut path: Vec<Uuid> = focused.via.iter().rev().copied().collect();
    path.push(id);
    Some(RowKey::Block(path))
}

fn deepest_shown(keys: &[RowKey], focused: &RowKey) -> Option<RowKey> {
    let RowKey::Block(path) = focused else {
        return keys.iter().find(|key| *key == focused).cloned();
    };
    (1..=path.len()).rev().find_map(|length| {
        let candidate = RowKey::Block(path[..length].to_vec());
        keys.iter().find(|key| **key == candidate).cloned()
    })
}

fn stray(
    shown: &Memo<Option<RowKey>>,
    buried: &Memo<Option<RowKey>>,
    tree: &NodeRef,
    scroll: &NodeRef,
) -> Option<Astray> {
    let key = shown.get_untracked()?;
    if buried.get_untracked().is_some() {
        return Some(Astray::Below);
    }
    let (Some(tree), Some(scroll)) = (tree.try_get(), scroll.try_get()) else {
        return None;
    };
    with_document(|document| {
        let node = tree_row_node::<RowKey>(document, tree, &key)?;
        let row = document.node_rect(node)?;
        let viewport = document.node_rect(scroll)?;
        if row.bottom() <= viewport.top() {
            return Some(Astray::Above);
        }
        if row.top() >= viewport.bottom() {
            return Some(Astray::Below);
        }
        None
    })
}

fn row_test_id(key: &RowKey) -> String {
    match key {
        RowKey::Block(path) => path
            .last()
            .map_or_else(|| "block".to_owned(), Uuid::to_string),
        RowKey::Orphans => "orphans".to_owned(),
        RowKey::Note(path) => format!("note.{}", path.len()),
    }
}

#[component]
fn TreeRow(
    editor: Editor,
    tree: Rc<FileTree>,
    picker: Rc<Picker>,
    held: Held,
    row: Memo<Option<Row>>,
    handle: TreeRowHandle<RowKey>,
) -> NodeId {
    let named = row_test_id(&handle.key);
    let TreeRowHandle {
        item,
        selected,
        focused,
        select,
        toggle,
        hover,
        ..
    } = handle;
    let rect = component_rect();
    let start = clone!(editor held row -> move || {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        held.set(Some(Carried {
            id,
            block_type: shown.block_type,
            source: shown.source,
            is_reference: shown.is_reference,
        }));
        editor.host().drag_block(id, shown.block_type);
    });
    let gesture: Rc<std::cell::Cell<Option<(Pos2, bool)>>> = Rc::default();
    let pressed = clone!(gesture -> move |press: PointerPress| {
        gesture.set(Some((press.pos, false)));
    });
    let moved = clone!(gesture -> move |at: PointerPress| {
        let Some((origin, started)) = gesture.get() else {
            return;
        };
        if started || (at.pos - origin).length() < DRAG_THRESHOLD {
            return;
        }
        gesture.set(Some((origin, true)));
        start();
    });
    let settled = clone!(gesture -> move |active: bool| {
        if !active {
            gesture.set(None);
        }
    });
    let chose_row = clone!(gesture -> move || {
        if gesture.get().is_none_or(|(_, dragged)| !dragged) {
            select();
        }
    });
    let (hovered, set_hovered) = create_signal(false);
    let arriving = arrival(&editor, rect, held, row.clone());
    let welcome = create_memo(clone!(arriving -> move || arriving.get() == Some(true)));
    let arriving_here = create_memo(clone!(arriving -> move || arriving.get().is_some()));
    let indent = create_memo(clone!(item -> move || {
        ItemSize::Fixed(item.get().depth as f32 * ROW_INDENT)
    }));
    let label = create_memo(clone!(row -> move || {
        row.get().map(|row| row.label).unwrap_or_default()
    }));
    let glyph = create_memo(clone!(row -> move || {
        row.get().map(|row| row.glyph).unwrap_or_default()
    }));
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let generated = create_memo(clone!(row -> move || {
        row.get().is_some_and(|row| row.dynamic_artifact)
    }));
    let access = create_memo(clone!(row -> move || {
        row.get()
            .and_then(|row| access_marker(row.access))
            .unwrap_or_default()
            .to_owned()
    }));
    let access_label = create_memo(clone!(row -> move || {
        row.get().map(|row| access_hint(row.access)).unwrap_or_default().to_owned()
    }));
    let restricted = create_memo(clone!(access -> move || !access.get().is_empty()));
    let can_add = create_memo(clone!(row -> move || row.get().is_some_and(|row| row.can_add)));
    let muted = create_memo(clone!(row -> move || {
        row.get().is_none_or(|row| row.automatic || row.id.is_none())
    }));

    let add_child = clone!(picker editor row -> move || {
        let Some(id) = row.get_untracked().and_then(|row| row.id) else {
            return;
        };
        picker.open(&editor, Some(id), [id].into_iter().collect::<HashSet<Uuid>>());
    });
    let chose = menu_action(editor.clone(), Rc::clone(&tree), picker, row.clone());
    let add = create_memo(clone!(row -> move || !row.get().is_some_and(|row| row.can_add)));
    let edit = create_memo(clone!(row -> move || !row.get().is_some_and(|row| row.can_edit)));
    let unlinkable = create_memo(clone!(row -> move || {
        !row.get().is_some_and(|row| row.is_reference && row.unlink.is_ok())
    }));
    let deletable =
        create_memo(clone!(row -> move || !row.get().is_some_and(|row| row.can_delete)));
    let delete_label = create_memo(clone!(row -> move || {
        match row.get().is_some_and(|row| row.is_reference) {
            true => "Remove link".to_owned(),
            false => "Delete".to_owned(),
        }
    }));
    let rooted = create_memo(clone!(row edit -> move || {
        edit.get() || row.get().is_some_and(|row| row.parent == BlockParent::Root)
    }));
    let orphaned = create_memo(clone!(row edit -> move || {
        edit.get() || row.get().is_some_and(|row| row.parent == BlockParent::Orphaned)
    }));
    let items = view! {
        <MenuItem label="Add" disabled={add} />
        <MenuItem label="Set parent" disabled={edit.clone()}>
            <MenuItem label="Root" disabled={rooted} />
            <MenuItem label="Orphaned" disabled={orphaned} />
        </MenuItem>
        <MenuItem label="Rename" disabled={edit.clone()} />
        <MenuItem label="Share" disabled={edit} />
        <MenuItem label="Unlink" disabled={unlinkable} />
        <MenuItem label={delete_label} disabled={deletable} />
    };
    let theme = use_theme();
    let glyph_color = theme.text_muted.clone();
    let generated_color = theme.text_muted.clone();
    let access_color = theme.text_muted.clone();
    let color = create_memo(clone!(theme muted -> move || match muted.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    let fill = create_memo(clone!(theme selected hovered -> move || {
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent_soft.get(),
            (false, true) => theme.hover.get(),
            (false, false) => Color32::TRANSPARENT,
        }
    }));
    let outline = create_memo(clone!(theme welcome arriving_here focused -> move || {
        match (arriving_here.get(), welcome.get(), focused.get()) {
            (true, true, _) => theme.accent.get(),
            (true, false, _) => theme.danger.get(),
            (false, _, _) => theme.accent.get(),
        }
    }));
    let outlined = create_memo(clone!(arriving_here focused -> move || {
        arriving_here.get() || focused.get()
    }));
    view! {
        <Frame height=ROW_HEIGHT>
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Spacer @sizing={indent} />
                <Chevron item={item} toggle={toggle} named={named.clone()} />
                <ContextMenu @sizing=ItemSize::Percent(100.0) items={items} on_select={chose}>
                    <Frame
                        @test_id={format!("file-tree.row.{named}")}
                        height=ROW_HEIGHT
                        color={fill}
                        outline={outline}
                        outline_width=1.0
                        outline_visible={outlined}
                        radius=RADIUS
                        padding_horizontal=4.0
                    >
                        <ClickCatcher
                            cursor=CursorIcon::PointingHand
                            on_press={pressed}
                            on_drag={moved}
                            on_click={chose_row}
                            on_active_change={settled}
                            on_hover_change={move |over: bool| {
                                set_hovered.set(over);
                                hover(over);
                            }}
                        >
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=ROW_SPACING
                            >
                                <Show condition={has_glyph}>
                                    <IconSized
                                        glyph={glyph}
                                        font_size=FONT_SMALL
                                        color={glyph_color}
                                    />
                                </Show>
                                <Body
                                    @sizing=ItemSize::Percent(100.0)
                                    content={label}
                                    color={color}
                                />
                                <Show condition={generated}>
                                    <Tooltip label="Generated from another block">
                                        <IconSized
                                            glyph={ICON_AUTO_AWESOME.to_owned()}
                                            font_size=FONT_SMALL
                                            color={generated_color}
                                        />
                                    </Tooltip>
                                </Show>
                                <Show condition={restricted}>
                                    <Tooltip label={access_label}>
                                        <IconSized
                                            glyph={access}
                                            font_size=FONT_SMALL
                                            color={access_color}
                                        />
                                    </Tooltip>
                                </Show>
                                <Show condition={can_add}>
                                    <AddChild shown={hovered} named={named} on_click={add_child} />
                                </Show>
                            </List>
                        </ClickCatcher>
                    </Frame>
                </ContextMenu>
            </List>
        </Frame>
    }
}

#[component]
fn Chevron(item: Memo<TreeItem>, toggle: Rc<dyn Fn()>, named: String) -> NodeId {
    let expandable = create_memo(clone!(item -> move || item.get().expandable));
    let glyph = create_memo(clone!(item -> move || {
        match (item.get().expandable, item.get().expanded) {
            (false, _) => String::new(),
            (true, true) => ICON_KEYBOARD_ARROW_DOWN.to_owned(),
            (true, false) => ICON_KEYBOARD_ARROW_RIGHT.to_owned(),
        }
    }));
    let marked = create_memo(clone!(item -> move || item.get().marked));
    let label = create_memo(clone!(item -> move || match item.get().expanded {
        true => "Collapse".to_owned(),
        false => "Expand".to_owned(),
    }));
    view! {
        <Frame width=CHEVRON_WIDTH>
            <List spacing=0.0>
                <Show condition={expandable}>
                    <unstyled::Button
                        @test_id={format!("file-tree.chevron.{named}")}
                        tab_stop=false
                        accessibility={chevron_accessibility(item)}
                        on_click={move || toggle()}
                        content={move |button: ButtonHandle| view! {
                            <ChevronFace
                                handle={button}
                                glyph={glyph}
                                marked={marked}
                                label={label}
                            />
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}

fn chevron_accessibility(item: Memo<TreeItem>) -> Memo<AccessNode> {
    create_memo(move || {
        let mut node = AccessNode::new(Role::Button);
        node.set_expanded(item.get().expanded);
        node.set_label(match item.get().expanded {
            true => "Collapse",
            false => "Expand",
        });
        node
    })
}

#[component]
fn ChevronFace(
    handle: ButtonHandle,
    glyph: Memo<String>,
    marked: Memo<bool>,
    label: Memo<String>,
) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme marked hovered active -> move || {
        match (active.get(), marked.get(), hovered.get()) {
            (true, _, _) => theme.pressed.get(),
            (false, true, _) => theme.accent_soft.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => Color32::TRANSPARENT,
        }
    }));
    let outlined = create_memo(clone!(marked focused -> move || marked.get() || focused.get()));
    let color = create_memo(clone!(theme marked hovered -> move || {
        match marked.get() || hovered.get() {
            true => theme.text.get(),
            false => theme.text_muted.get(),
        }
    }));
    view! {
        <Tooltip label={label}>
            <Frame
                width=CHEVRON_WIDTH
                height=ROW_HEIGHT
                color={fill}
                outline={theme.accent.clone()}
                outline_width=1.0
                outline_visible={outlined}
                radius=3
            >
                <IconSized glyph={glyph} font_size=FONT_SMALL color={color} />
            </Frame>
        </Tooltip>
    }
}

#[component]
fn AddChild(
    shown: ReadSignal<bool>,
    named: String,
    on_click: block_editor_plugin::beui::reactive::ClickCallback,
) -> NodeId {
    view! {
        <Frame width=ADD_WIDTH>
            <List spacing=0.0>
                <Show condition={shown}>
                    <unstyled::Button
                        @test_id={format!("file-tree.add.{named}")}
                        tab_stop=false
                        capture_presses=true
                        accessibility={add_child_accessibility()}
                        on_click={move || on_click.call()}
                        content={move |button: ButtonHandle| view! {
                            <AddChildFace handle={button} />
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}

fn add_child_accessibility() -> AccessNode {
    let mut node = AccessNode::new(Role::Button);
    node.set_label("Add a child");
    node
}

#[component]
fn AddChildFace(handle: ButtonHandle) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let color = create_memo(clone!(theme hovered active -> move || {
        match hovered.get() || active.get() {
            true => theme.text.get(),
            false => theme.text_muted.get(),
        }
    }));
    let fill = create_memo(clone!(theme active -> move || match active.get() {
        true => theme.pressed.get(),
        false => Color32::TRANSPARENT,
    }));
    view! {
        <Tooltip label="Add a child">
            <Frame
                width=ADD_WIDTH
                height=ADD_WIDTH
                color={fill}
                outline={theme.accent.clone()}
                outline_width=1.0
                outline_visible={focused}
                radius=3
            >
                <IconSized glyph={ICON_ADD.to_owned()} font_size=FONT_SMALL color={color} />
            </Frame>
        </Tooltip>
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Carried {
    id: Uuid,
    block_type: Uuid,
    source: BlockSource,
    is_reference: bool,
}

pub(crate) type Held = Rc<std::cell::Cell<Option<Carried>>>;

fn arrival(
    editor: &Editor,
    rect: block_editor_plugin::beui::reactive::ReadSignal<block_editor_plugin::beui::Rect>,
    held: Held,
    row: Memo<Option<Row>>,
) -> Memo<Option<bool>> {
    let (state, set_state) = create_signal(None::<bool>);
    let drag = editor.drag();
    let here = editor.clone();
    editor.each_frame(move || {
        let (Some(drag), Some(shown)) = (drag.get_untracked(), row.get_untracked()) else {
            set_state.set(None);
            return;
        };
        let (Some(id), Some(carried)) = (shown.id, held.get()) else {
            set_state.set(None);
            return;
        };
        if carried.id != drag.block_id || !rect.get_untracked().contains(drag.position) {
            set_state.set(None);
            return;
        }
        let accepts = shown.can_add && carried.id != id && carried.source != BlockSource::Block(id);
        if drag.dropped {
            set_state.set(None);
            held.set(None);
            if accepts {
                here.host().move_block(
                    carried.id,
                    carried.block_type,
                    carried.source,
                    id,
                    carried.is_reference,
                );
            }
            return;
        }
        here.accept_drag(accepts);
        set_state.set(Some(accepts));
    });
    create_memo(move || state.get())
}

fn menu_action(
    editor: Editor,
    tree: Rc<FileTree>,
    picker: Rc<Picker>,
    row: Memo<Option<Row>>,
) -> impl Fn(Vec<usize>) + 'static {
    move |path: Vec<usize>| {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        match path.as_slice() {
            [0] => picker.open(
                &editor,
                Some(id),
                [id].into_iter().collect::<HashSet<Uuid>>(),
            ),
            [1, 0] => tree.client().set_block_parent(id, BlockParent::Root),
            [1, 1] => tree.client().set_block_parent(id, BlockParent::Orphaned),
            [2] => editor.host().rename_block(id),
            [3] => editor.host().share_block(id),
            [4] => {
                if let Some(container) = shown.container {
                    editor.host().unlink_block(id, container);
                }
            }
            [5] => {
                editor
                    .host()
                    .delete_block(id, shown.block_type, shown.source, shown.is_reference)
            }
            _ => {}
        }
    }
}

pub(crate) struct Picker {
    picker: std::cell::RefCell<BlockPicker>,
    target: std::cell::Cell<Option<Uuid>>,
    error: block_editor_plugin::beui::reactive::ReadSignal<Option<String>>,
    set_error: block_editor_plugin::beui::reactive::WriteSignal<Option<String>>,
}

impl Picker {
    pub(crate) fn error(&self) -> Memo<Option<String>> {
        let error = self.error.clone();
        create_memo(move || error.get())
    }

    pub(crate) fn open(&self, editor: &Editor, parent: Option<Uuid>, excluded: HashSet<Uuid>) {
        self.set_error.set(None);
        self.target.set(parent);
        self.picker.borrow_mut().open(
            editor.host(),
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: excluded.into_iter().map(Uuid::into_bytes).collect(),
                templates: false,
            },
        );
    }
}

fn picker(editor: &Editor, tree: Rc<FileTree>) -> Rc<Picker> {
    let (error, set_error) = create_signal(None::<String>);
    let picker = Rc::new(Picker {
        picker: std::cell::RefCell::new(BlockPicker::default()),
        target: std::cell::Cell::new(None),
        error,
        set_error,
    });
    let polled = Rc::clone(&picker);
    let host = editor.host().clone();
    editor.each_frame(move || {
        let Some(result) = polled.picker.borrow_mut().poll(&host) else {
            return;
        };
        let target = polled.target.take();
        let picked = match result {
            Ok(picked) => picked,
            Err(error) => {
                polled.set_error.set(Some(error));
                return;
            }
        };
        tree.remember(picked.id, picked.block_type);
        match target {
            None => {
                if !picked.linked {
                    tree.client().set_block_parent(picked.id, BlockParent::Root);
                }
                host.open_block(picked.id, picked.block_type);
            }
            Some(parent) => {
                host.place_block(picked.id, picked.block_type, parent, picked.linked);
                host.open_block_via(picked.id, picked.block_type, parent);
            }
        }
    });
    picker
}
