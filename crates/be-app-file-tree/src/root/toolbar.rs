use std::collections::HashSet;

use be_sdk_beui::beui::icons::{
    ICON_ARROW_UP, ICON_BOXES, ICON_CHEVRON_DOWN, ICON_CIRCLE_USER_ROUND, ICON_FILE_PLUS,
    ICON_PLUS, ICON_SEARCH,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::unstyled;
use be_sdk_beui::beui::{Key, KeyPhase, NodeId, TextAlign};
use be_sdk_beui::{Editor, Toolbar};
use beui::Overflow;
use uuid::Uuid;

use super::rename::Renaming;
use super::rows::{RowKey, Tree as FileTree};
use super::ui::{Picker, add_to_section, row_test_id};
use super::zoom::Zoom;
use beui::{Align, Direction};

const SPACING: Space = Space::Sm;

#[derive(Clone, Copy)]
pub(crate) struct Adding {
    pub(crate) editor: Editor,
    pub(crate) tree: FileTree,
    pub(crate) zoom: Zoom,
    pub(crate) picker: Picker,
    pub(crate) renaming: Renaming,
    pub(crate) query: Signal<String>,
}

#[component]
pub(crate) fn FilesToolbar(adding: Adding, shown: Memo<bool>) -> DynamicSegment<ListItem> {
    let Adding { editor, zoom, .. } = adding;
    let account = NodeRef::new();
    let focus = zoom.focus();
    let zoomed = create_memo(move || focus.with(Option::is_some));
    view! {
        <Toolbar shown={shown} fit=true>
            <ListItem grow=1.0>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <ListItem grow=1.0>
                            <FilterField zoom query={adding.query} />
                        </ListItem>
                        <NewButton adding />
                        <Button
                            icon_only=true
                            @node_ref={&account}
                            glyph=ICON_CIRCLE_USER_ROUND
                            label="Account and settings"
                            variant=ButtonVariant::Ghost
                            @test_id={"file-tree.host-menu"}
                            on_click={move || editor.show_host_menu(account)}
                        />
                    </List>
                    <Show condition={zoomed}>
                        <Location zoom />
                    </Show>
                </List>
            </ListItem>
        </Toolbar>
    }
}

#[component]
fn FilterField(zoom: Zoom, query: Signal<String>) -> NodeId {
    let focus = zoom.focus();
    let placeholder = create_memo(move || match focus.get() {
        Some(focus) => format!("Filter {}", focus.row.label),
        None => "Filter files".to_owned(),
    });
    let field = NodeRef::new();
    on_key(KeyAt::Node(field), KeyPhase::Capture, move |event| {
        let escaped = event
            .press()
            .is_some_and(|press| press.pressed && press.key == Key::Escape);
        let clears = escaped && !query.with_untracked(String::is_empty);
        if clears {
            query.set(String::new());
        }
        clears
    });
    view! {
        <TextInput
            @node_ref={&field}
            value={query}
            placeholder={placeholder}
            label="Filter files"
            glyph=ICON_SEARCH
            clearable=true
            @test_id={"file-tree.filter"}
        />
    }
}

#[component]
fn NewButton(adding: Adding) -> NodeId {
    let Adding {
        editor,
        tree,
        zoom,
        picker,
        renaming,
        query,
    } = adding;
    let (here, menu_button) = (NodeRef::new(), NodeRef::new());
    let focus = zoom.focus();
    let zoomed = create_memo(move || focus.with(Option::is_some));
    let addable = create_memo(move || {
        focus.with(|focus| {
            focus
                .as_ref()
                .and_then(|focus| focus.row.id.filter(|_| focus.row.can_add))
        })
    });
    let primary = create_memo(move || addable.with(Option::is_some));
    let menu = create_memo(move || !primary.get());
    let rows = tree.rows();
    let workspaces = create_memo(move || {
        rows.with(|rows| {
            rows.iter()
                .filter(|row| row.depth == 0 && row.is_workspace() && row.can_add && !row.deleted)
                .filter_map(|row| row.id)
                .collect::<Vec<_>>()
        })
    });
    let add_into = move |id: Uuid| {
        query.set(String::new());
        let key = rows.with_untracked(|rows| {
            rows.iter()
                .find(|row| row.id == Some(id) && row.depth == 0)
                .map(|row| (row.key.clone(), row.expandable && !row.expanded))
        });
        if let Some((key, true)) = key {
            tree.toggle(&key, Some(id), false);
        }
        editor.cancel_pick();
        add_to_section(editor, picker, id, menu_button);
    };
    let add_here = move || {
        query.set(String::new());
        if let Some(id) = addable.get_untracked() {
            picker.open(
                editor,
                id,
                [id].into_iter().collect::<HashSet<Uuid>>(),
                here,
            );
        }
    };
    let menu_open = create_signal(false);
    let opening = Binding::new(menu_open, move |open: bool| {
        menu_open.set(open);
        if open {
            editor.cancel_pick();
        }
    });
    let new_workspace = move || {
        query.set(String::new());
        super::ui::new_workspace(editor, renaming);
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
            <Show condition={primary}>
                <Button
                    @node_ref={&here}
                    glyph=ICON_PLUS
                    label="New"
                    variant=ButtonVariant::Primary
                    @test_id={"file-tree.new"}
                    on_click={add_here}
                />
            </Show>
            <Show condition={menu}>
                <MenuButton
                    @node_ref={&menu_button}
                    label="New"
                    icon_only=true
                    disabled={zoomed}
                    open={opening}
                    @test_id={"file-tree.new"}
                    leading={view! {
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=Space::Xs2
                        >
                            <Text align=TextAlign::Center>
                                <Icon glyph=ICON_PLUS />
                            </Text>
                            <Text size=TextSize::Xs color=TextColor::Muted align=TextAlign::Center>
                                <Icon glyph=ICON_CHEVRON_DOWN />
                            </Text>
                        </List>
                    }}
                    items={view! {
                        <ForEach keys={workspaces}>|id: Uuid| {
                            let row = tree.row(RowKey::Block(vec![id]));
                            let label = create_memo(move || {
                                format!("New in {}…", row.get().map(|row| row.label).unwrap_or_default())
                            });
                            view! {
                                <MenuItem
                                    label={label}
                                    glyph=ICON_FILE_PLUS
                                    row_test_id={format!("file-tree.new-in.{id}")}
                                    on_click={move || add_into(id)}
                                />
                            }
                        }</ForEach>
                        <MenuItem
                            label="New workspace"
                            glyph=ICON_BOXES
                            separated=true
                            row_test_id={"file-tree.new-workspace".to_owned()}
                            on_click={new_workspace}
                        />
                    }}
                />
            </Show>
        </List>
    }
}

#[component]
fn Location(zoom: Zoom) -> NodeId {
    let focus = zoom.focus();
    let trail = create_memo(move || {
        focus.with(|focus| {
            focus.as_ref().map_or_else(Vec::new, |focus| {
                focus.trail.iter().map(|(key, _)| key.clone()).collect()
            })
        })
    });
    let name = create_memo(move || focus.get().map(|focus| focus.row.label).unwrap_or_default());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Button
                icon_only=true
                size=ButtonSize::Compact
                @test_id={"file-tree.back"}
                glyph=ICON_ARROW_UP
                label="Up one level"
                on_click={move || zoom.up()}
            />
            <ForEach keys={trail}>|key: RowKey| {
                let shown = key.clone();
                let label = create_memo(move || {
                    focus.with(|focus| {
                        focus
                            .as_ref()
                            .and_then(|focus| focus.trail.iter().find(|(held, _)| *held == shown))
                            .map(|(_, label)| label.clone())
                            .unwrap_or_default()
                    })
                });
                let named = format!("{}.crumb", row_test_id(&key));
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Crumb
                            label={label}
                            named={named}
                            on_click={move || zoom.out_to(key.clone())}
                        />
                        <Text color=TextColor::Muted>"/"</Text>
                    </List>
                }
            }</ForEach>
            <ListItem shrink=1.0>
                <Text weight=Weight::Bold overflow=Overflow::Ellipsis @test_id={"file-tree.here"}>
                    {name}
                </Text>
            </ListItem>
        </List>
    }
}

#[component]
fn Crumb(label: Memo<String>, named: String, on_click: Callback<()>) -> NodeId {
    view! {
        <unstyled::Button label={label} on_click={on_click} @test_id={named}>|handle|
            <CrumbFace handle />
        </unstyled::Button>
    }
}

#[component]
fn CrumbFace(handle: unstyled::ButtonHandle) -> NodeId {
    let unstyled::ButtonHandle {
        hovered,
        focused,
        label,
        ..
    } = handle;
    let underlined = create_memo(move || hovered.get() || focused.get());
    view! {
        <FocusRing focused radius=Radius::Xs offset=Stroke::Focus>
            <Text color=TextColor::Muted underline={underlined} overflow=Overflow::Ellipsis>
                {label}
            </Text>
        </FocusRing>
    }
}
