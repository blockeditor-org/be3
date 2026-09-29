use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use block_editor_beui::be_block::{BlockContent, FolderContent, WorkspaceUiContent};
use block_editor_beui::beui::icons::{
    ICON_ADD, ICON_ARROW_BACK, ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_DELETE,
    ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_FILE_DOWNLOAD, ICON_HISTORY, ICON_INFO, ICON_LINK_OFF,
    ICON_MORE_VERT, ICON_MOVE_UP, ICON_NOTE_ADD, ICON_OPEN_IN_NEW, ICON_SEARCH, ICON_SEARCH_OFF,
    ICON_SHARE,
};
use block_editor_beui::beui::reactive::{
    Align, BackHandler, Direction, Dynamic, ForEach, Frame, ItemSize, List, Memo, NodeRef,
    ReadSignal, Show, Spacer, Text, WriteSignal, clone, component, create_memo, create_signal,
    view,
};
use block_editor_beui::beui::styled::theme::{CARD_RADIUS, FONT_BODY, RADIUS};
use block_editor_beui::beui::styled::{
    ActionRow, Button, ButtonVariant, Caption, Heading, Icon, IconButton, IconSized, ListRow,
    ModalSheet, Scroll, TextInput, Title, use_theme,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::root_settings::RootSetting;
use block_editor_beui::{
    BlockParent, BlockQuery, BottomDock, ChildTarget, Editor, watch_block_label,
};
use uuid::Uuid;

use super::export::{Exporter, exportable};
use super::rows::{Inspection, Placement, Row, RowKey, Tree as FileTree, describe};
use super::ui::{Picker, menu_action};

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;
const TILE_SIDE: f32 = 40.0;
const TILE_GLYPH: f32 = 22.0;
const ROW_INNER_SPACING: f32 = 14.0;
const RECENT_WIDTH: f32 = 132.0;
const RECENT_HEIGHT: f32 = 96.0;
const RECENT_SHOWN: usize = 10;
const DOCK_ROOM: f32 = 88.0;
const SEARCH_HEIGHT: f32 = 44.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Crumb {
    id: Uuid,
    block_type: Uuid,
    name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Level {
    Root,
    Folder(Vec<Crumb>),
    Deleted,
}

impl Level {
    fn query(&self) -> BlockQuery {
        match self {
            Self::Root => BlockQuery::Roots,
            Self::Folder(crumbs) => crumbs
                .last()
                .map_or(BlockQuery::Roots, |crumb| BlockQuery::References(crumb.id)),
            Self::Deleted => BlockQuery::Detached,
        }
    }

    fn container(&self) -> Option<&Crumb> {
        match self {
            Self::Folder(crumbs) => crumbs.last(),
            Self::Root | Self::Deleted => None,
        }
    }

    fn up(&self) -> Self {
        match self {
            Self::Folder(crumbs) if crumbs.len() > 1 => {
                Self::Folder(crumbs[..crumbs.len() - 1].to_vec())
            }
            Self::Root | Self::Folder(_) | Self::Deleted => Self::Root,
        }
    }

    fn inside(&self, row: &Row) -> Option<Self> {
        let id = row.id?;
        let crumb = Crumb {
            id,
            block_type: row.block_type,
            name: row.label.clone(),
        };
        Some(match self {
            Self::Folder(crumbs) => {
                let mut crumbs = crumbs.clone();
                crumbs.push(crumb);
                Self::Folder(crumbs)
            }
            Self::Root | Self::Deleted => Self::Folder(vec![crumb]),
        })
    }
}

fn drills_in(row: &Row) -> bool {
    row.block_type == FolderContent::CONTENT_TYPE && !row.is_reference
}

#[component]
pub(crate) fn PhoneFiles(
    editor: Editor,
    tree: Rc<FileTree>,
    picker: Rc<Picker>,
    exporter: Rc<Exporter>,
    inspect: WriteSignal<Option<Inspection>>,
    covered: Memo<bool>,
) -> NodeId {
    let (level, set_level) = create_signal(Level::Root);
    let (query, set_query) = create_signal(String::new());
    let (acting, set_acting) = create_signal(None::<Row>);
    let searching = create_memo(clone!(query -> move || !query.get().trim().is_empty()));
    let browsing = create_memo(clone!(searching -> move || !searching.get()));
    let at_root = create_memo(clone!(level browsing -> move || {
        browsing.get() && level.get() == Level::Root
    }));
    let nested = create_memo(clone!(level -> move || level.get() != Level::Root));
    let can_create = create_memo(clone!(level browsing acting -> move || {
        browsing.get()
            && level.get() != Level::Deleted
            && acting.get().is_none()
            && !covered.get()
    }));
    let page = NodeRef::new();
    let creating = clone!(editor picker level tree -> move || {
        let parent = level.with_untracked(|level| {
            level.container().filter(|crumb| {
                editor.block_types().child_edits(crumb.block_type).add
                    && tree.blocks().access(crumb.id).can_edit()
            })
            .map(|crumb| crumb.id)
        });
        let excluded = parent.into_iter().collect::<HashSet<Uuid>>();
        picker.open_placed(&editor, parent, excluded);
    });
    let rising = clone!(level set_level -> move || set_level.set(level.get_untracked().up()));
    let rows_editor = editor.clone();
    let rows_tree = Rc::clone(&tree);
    let rows_level = set_level.clone();
    let rows_acting = set_acting.clone();
    let recents_editor = editor.clone();
    let header_level = level.clone();
    let header_set = set_level.clone();
    view! {
        <BackHandler enabled={nested} on_back={rising}>
            <List spacing=0.0>
                <Header level={header_level} set_level={header_set} query set_query />
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&page}>
                    <List spacing=0.0>
                        <Show condition={searching}>
                            <SearchSoon @sizing=ItemSize::Percent(100.0) />
                        </Show>
                        <Show condition={browsing}>
                            <Scroll @sizing=ItemSize::Percent(100.0)>
                                <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                                    <List spacing=ROW_SPACING>
                                        <Show condition={at_root}>
                                            <Recents editor={recents_editor} />
                                        </Show>
                                        <Dynamic value={level}>
                                            {move |level: Level| {
                                                let editor = rows_editor.clone();
                                                let tree = Rc::clone(&rows_tree);
                                                let set_level = rows_level.clone();
                                                let set_acting = rows_acting.clone();
                                                view! {
                                                    <LevelRows
                                                        @sizing=ItemSize::Intrinsic
                                                        editor
                                                        tree
                                                        level
                                                        set_level
                                                        set_acting
                                                    />
                                                }
                                            }}
                                        </Dynamic>
                                        <Frame height=DOCK_ROOM />
                                    </List>
                                </Frame>
                            </Scroll>
                        </Show>
                    </List>
                </Frame>
                <BottomDock anchor={page} open={can_create} name="file-tree.dock">
                    <Button
                        @test_id={"file-tree.new"}
                        label="New"
                        glyph={ICON_ADD.to_owned()}
                        variant=ButtonVariant::Primary
                        on_click={creating}
                    />
                </BottomDock>
                <RowActions
                    editor
                    tree
                    picker
                    exporter
                    inspect
                    acting
                    set_acting
                    set_level
                />
            </List>
        </BackHandler>
    }
}

#[component]
fn Header(
    level: ReadSignal<Level>,
    set_level: WriteSignal<Level>,
    query: ReadSignal<String>,
    set_query: WriteSignal<String>,
) -> NodeId {
    let theme = use_theme();
    let top = create_memo(clone!(level -> move || level.get() == Level::Root));
    let inside = create_memo(clone!(top -> move || !top.get()));
    let name = create_memo(clone!(level -> move || match level.get() {
        Level::Root => "Files".to_owned(),
        Level::Folder(crumbs) => crumbs.last().map(|crumb| crumb.name.clone()).unwrap_or_default(),
        Level::Deleted => "Recently deleted".to_owned(),
    }));
    let path = create_memo(clone!(level -> move || match level.get() {
        Level::Folder(crumbs) => std::iter::once("Files".to_owned())
            .chain(crumbs[..crumbs.len().saturating_sub(1)].iter().map(|crumb| crumb.name.clone()))
            .collect::<Vec<_>>()
            .join(" / "),
        Level::Root | Level::Deleted => "Files".to_owned(),
    }));
    let up = clone!(level -> move || set_level.set(level.get_untracked().up()));
    let typed = query.clone();
    let clearable = create_memo(clone!(query -> move || !query.get().is_empty()));
    let changing = set_query.clone();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=PADDING>
                <Show condition={top}>
                    <Frame padding_horizontal=PADDING padding_vertical=4.0>
                        <Title content="Files" />
                    </Frame>
                </Show>
                <Show condition={inside}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                        <IconButton
                            @test_id={"file-tree.back"}
                            glyph={ICON_ARROW_BACK.to_owned()}
                            label="Up one folder"
                            on_click={up}
                        />
                        <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                            <Caption content={path} ellipsis=true />
                            <Heading content={name} />
                        </List>
                    </List>
                </Show>
                <Frame
                    height=SEARCH_HEIGHT
                    radius=CARD_RADIUS
                    color={theme.surface.clone()}
                    outline={theme.border.clone()}
                    outline_width=1.0
                    outline_visible=true
                    padding_horizontal=12.0
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                        <Icon glyph={ICON_SEARCH.to_owned()} color={theme.text_muted.clone()} />
                        <TextInput
                            @sizing=ItemSize::Percent(100.0)
                            @test_id={"file-tree.search"}
                            value={typed}
                            placeholder="Search files"
                            label="Search files"
                            plain=true
                            on_change={move |value: String| changing.set(value)}
                        />
                        <Show condition={clearable}>
                            <IconButton
                                @test_id={"file-tree.search.clear"}
                                glyph={ICON_CLOSE.to_owned()}
                                label="Clear search"
                                on_click={move || set_query.set(String::new())}
                            />
                        </Show>
                    </List>
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn SearchSoon() -> NodeId {
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=32.0 padding_vertical=48.0 @test_id={"file-tree.search.todo"}>
            <List spacing=10.0 align=Align::Center>
                <IconSized
                    glyph={ICON_SEARCH_OFF.to_owned()}
                    font_size=40.0
                    color={theme.text_muted.clone()}
                />
                <Heading content="Search is coming soon" />
                <Caption
                    content="It isn't built yet. Open folders to find a file for now."
                    wrap=true
                />
            </List>
        </Frame>
    }
}

#[component]
fn Recents(editor: Editor) -> NodeId {
    let setting = RefCell::new(RootSetting::<WorkspaceUiContent>::default());
    let finding = editor.clone();
    let block = create_memo(move || {
        setting
            .borrow_mut()
            .find(&finding, finding.host().client_id())
    });
    let own = editor.block_id();
    let recents = editor
        .related_content::<WorkspaceUiContent>(block)
        .project(move |content| {
            content
                .root()
                .recent_blocks()
                .into_iter()
                .filter(|(id, _)| *id != own)
                .take(RECENT_SHOWN)
                .collect::<Vec<(Uuid, Uuid)>>()
        });
    let keys = create_memo(clone!(recents -> move || {
        recents.with(|recents| recents.iter().map(|(id, _)| *id).collect::<Vec<Uuid>>())
    }));
    let any = create_memo(clone!(keys -> move || !keys.get().is_empty()));
    view! {
        <List spacing=0.0>
            <Show condition={any}>
                <List spacing=6.0>
                    <Frame padding_horizontal=PADDING padding_vertical=4.0>
                        <Caption content="Recent" />
                    </Frame>
                    <Scroll direction=Direction::Horizontal>
                        <ForEach keys={keys}>
                            {move |id: Uuid| {
                                let block_type = recents.with_untracked(|recents| {
                                    recents
                                        .iter()
                                        .find(|(other, _)| *other == id)
                                        .map_or_else(Uuid::nil, |(_, block_type)| *block_type)
                                });
                                let editor = editor.clone();
                                view! {
                                    <RecentCard editor id block_type />
                                }
                            }}
                        </ForEach>
                    </Scroll>
                    <Frame padding_horizontal=PADDING padding_vertical=4.0>
                        <Caption content="All files" />
                    </Frame>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RecentCard(editor: Editor, id: Uuid, block_type: Uuid) -> NodeId {
    let theme = use_theme();
    let target = create_memo(move || Some(ChildTarget::new(id, block_type)));
    let shown = watch_block_label(&editor, target);
    let resolved = create_memo(clone!(shown -> move || shown.with(|shown| shown.resolved)));
    let name = create_memo(clone!(shown -> move || shown.with(|shown| shown.name.clone())));
    let glyph = create_memo(clone!(shown -> move || shown.with(|shown| shown.glyph.clone())));
    let kind = create_memo(clone!(shown -> move || shown.with(|shown| shown.type_name.clone())));
    let host = editor.host().clone();
    view! {
        <Frame visible={resolved} padding_horizontal=4.0>
            <Frame
                width=RECENT_WIDTH
                height=RECENT_HEIGHT
                radius=CARD_RADIUS
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
            >
                <ListRow
                    @test_id={format!("file-tree.recent.{id}")}
                    on_click={move || host.open_block(id, block_type)}
                >
                    <Frame height={RECENT_HEIGHT - 8.0} padding_vertical=6.0>
                        <List spacing=6.0>
                            <Icon glyph color={theme.accent.clone()} />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Text
                                string={name}
                                font_size=FONT_BODY
                                color={theme.text.clone()}
                                ellipsis=true
                            />
                            <Caption content={kind} ellipsis=true />
                        </List>
                    </Frame>
                </ListRow>
            </Frame>
        </Frame>
    }
}

#[component]
fn LevelRows(
    editor: Editor,
    tree: Rc<FileTree>,
    level: Level,
    set_level: WriteSignal<Level>,
    set_acting: WriteSignal<Option<Row>>,
) -> NodeId {
    let listed = editor.watch_blocks(level.query());
    let describing = tree.clone();
    let types = editor.clone();
    let described_level = level.clone();
    let rows = create_memo(clone!(listed -> move || {
        let listed = listed.get()?;
        let types = types.block_types();
        let mut block_types: HashMap<Uuid, Uuid> = describing.block_types();
        if let Level::Folder(crumbs) = &described_level {
            block_types.extend(crumbs.iter().map(|crumb| (crumb.id, crumb.block_type)));
        }
        block_types.extend(listed.iter().map(|info| (info.id, info.block_type)));
        let path: Vec<Uuid> = match &described_level {
            Level::Folder(crumbs) => crumbs.iter().map(|crumb| crumb.id).collect(),
            Level::Root | Level::Deleted => Vec::new(),
        };
        let container = described_level.container().map(|crumb| crumb.id);
        Some(
            listed
                .iter()
                .map(|info| {
                    let mut key = path.clone();
                    key.push(info.id);
                    describe(
                        describing.blocks(),
                        types.as_ref(),
                        &block_types,
                        info,
                        Placement {
                            key: RowKey::Block(key),
                            depth: 0,
                            container,
                            expanded: false,
                        },
                        |id| id.to_string(),
                    )
                })
                .collect::<Vec<Row>>(),
        )
    }));
    let loading = create_memo(clone!(rows -> move || rows.with(Option::is_none)));
    let empty = create_memo(clone!(rows -> move || {
        rows.with(|rows| rows.as_ref().is_some_and(Vec::is_empty))
    }));
    let keys = create_memo(clone!(rows -> move || {
        rows.with(|rows| {
            rows.iter()
                .flatten()
                .filter_map(|row| row.id)
                .collect::<Vec<Uuid>>()
        })
    }));
    let empty_text = match &level {
        Level::Root => "Nothing here yet. Tap New to make your first file.",
        Level::Folder(_) => "This folder is empty. Tap New to add a file to it.",
        Level::Deleted => "Nothing has been deleted.",
    };
    let at_root = level == Level::Root;
    let deleting = set_level.clone();
    view! {
        <List spacing=ROW_SPACING>
            <Show condition={loading}>
                <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                    <Caption content="Loading…" />
                </Frame>
            </Show>
            <Show condition={empty}>
                <Frame padding_horizontal=PADDING padding_vertical=24.0 @test_id={"file-tree.empty"}>
                    <Caption content={empty_text} wrap=true />
                </Frame>
            </Show>
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let row = create_memo(clone!(rows -> move || {
                        rows.with(|rows| {
                            rows.iter().flatten().find(|row| row.id == Some(id)).cloned()
                        })
                    }));
                    let editor = editor.clone();
                    let level = level.clone();
                    let set_level = set_level.clone();
                    let set_acting = set_acting.clone();
                    view! {
                        <PhoneRow editor row level set_level set_acting />
                    }
                }}
            </ForEach>
            <Show condition={at_root}>
                <ActionRow
                    @test_id={"file-tree.deleted"}
                    label="Recently deleted"
                    glyph={ICON_HISTORY.to_owned()}
                    on_click={move || deleting.set(Level::Deleted)}
                />
            </Show>
        </List>
    }
}

#[component]
fn PhoneRow(
    editor: Editor,
    row: Memo<Option<Row>>,
    level: Level,
    set_level: WriteSignal<Level>,
    set_acting: WriteSignal<Option<Row>>,
) -> NodeId {
    let theme = use_theme();
    let id = row
        .get_untracked()
        .and_then(|row| row.id)
        .map(|id| id.to_string())
        .unwrap_or_default();
    let open_id = format!("file-tree.{id}.open");
    let inside_id = format!("file-tree.{id}.inside");
    let more_id = format!("file-tree.{id}.more");
    let name = create_memo(clone!(row -> move || row.get().map(|row| row.label).unwrap_or_default()));
    let glyph = create_memo(clone!(row -> move || row.get().map(|row| row.glyph).unwrap_or_default()));
    let types = editor.clone();
    let detail = create_memo(clone!(row -> move || {
        let Some(row) = row.get() else {
            return String::new();
        };
        let count = row
            .inspection
            .as_ref()
            .map(|inspection| inspection.references.clone())
            .unwrap_or_default();
        match drills_in(&row) {
            true => match count.as_str() {
                "1" => "1 item".to_owned(),
                count => format!("{count} items"),
            },
            false => types
                .block_types()
                .display_name(row.block_type)
                .unwrap_or("Block")
                .to_owned(),
        }
    }));
    let muted = create_memo(clone!(row -> move || row.get().is_none_or(|row| row.automatic)));
    let color = create_memo(clone!(theme muted -> move || match muted.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    let folder = create_memo(clone!(row -> move || row.get().is_some_and(|row| drills_in(&row))));
    let holds = create_memo(clone!(row -> move || {
        row.get().is_some_and(|row| row.expandable && !drills_in(&row))
    }));
    let host = editor.host().clone();
    let entering = clone!(level set_level row -> move || {
        if let Some(next) = row.get_untracked().and_then(|row| level.inside(&row)) {
            set_level.set(next);
        }
    });
    let tapped = clone!(entering row -> move || {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        if drills_in(&shown) {
            entering();
            return;
        }
        let (Some(id), true) = (shown.id, shown.access.can_view()) else {
            return;
        };
        match shown.container {
            Some(container) => host.open_block_via(id, shown.block_type, container),
            None => host.open_block(id, shown.block_type),
        }
    });
    let more = clone!(row -> move || set_acting.set(row.get_untracked()));
    let muted_icon = theme.text_muted.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
            <ListRow
                @sizing=ItemSize::Percent(100.0)
                @test_id={open_id}
                on_click={tapped}
            >
                <Frame padding_vertical=6.0>
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_INNER_SPACING>
                        <Frame
                            width=TILE_SIDE
                            height=TILE_SIDE
                            radius=RADIUS
                            color={theme.surface_raised.clone()}
                        >
                            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                                <Spacer @sizing=ItemSize::Percent(50.0) />
                                <IconSized glyph font_size=TILE_GLYPH color={theme.accent.clone()} />
                                <Spacer @sizing=ItemSize::Percent(50.0) />
                            </List>
                        </Frame>
                        <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                            <Text string={name} font_size=FONT_BODY color={color} ellipsis=true />
                            <Caption content={detail} ellipsis=true />
                        </List>
                        <Show condition={folder}>
                            <Icon glyph={ICON_CHEVRON_RIGHT.to_owned()} color={muted_icon} />
                        </Show>
                    </List>
                </Frame>
            </ListRow>
            <Show condition={holds}>
                <IconButton
                    @test_id={inside_id}
                    glyph={ICON_CHEVRON_RIGHT.to_owned()}
                    label="Show what it holds"
                    on_click={entering}
                />
            </Show>
            <IconButton
                @test_id={more_id}
                glyph={ICON_MORE_VERT.to_owned()}
                label="More actions"
                on_click={more}
            />
        </List>
    }
}

#[component]
fn RowActions(
    editor: Editor,
    tree: Rc<FileTree>,
    picker: Rc<Picker>,
    exporter: Rc<Exporter>,
    inspect: WriteSignal<Option<Inspection>>,
    acting: ReadSignal<Option<Row>>,
    set_acting: WriteSignal<Option<Row>>,
    set_level: WriteSignal<Level>,
) -> NodeId {
    let open = create_memo(clone!(acting -> move || acting.get().is_some()));
    let row = create_memo(clone!(acting -> move || acting.get()));
    let adding = Rc::clone(&picker);
    let chosen = Rc::new(menu_action(
        editor.clone(),
        tree,
        picker,
        exporter,
        inspect,
        row.clone(),
    ));
    let add_editor = editor.clone();
    let add_inside = clone!(row set_acting -> move || {
        let Some(id) = row.get_untracked().and_then(|row| row.id) else {
            return;
        };
        set_acting.set(None);
        adding.open_placed(&add_editor, Some(id), [id].into_iter().collect());
    });
    let off = |test: fn(&Row) -> bool| {
        let row = row.clone();
        create_memo(move || !row.get().is_some_and(|row| test(&row)))
    };
    let unviewable = off(|row| row.access.can_view());
    let unaddable = off(|row| row.can_add);
    let uneditable = off(|row| row.can_edit);
    let unshareable = off(|row| row.can_edit);
    let unmovable = off(|row| row.can_edit && row.parent != BlockParent::Root);
    let unlinkable = off(|row| row.is_reference && row.unlink.is_ok());
    let undeletable = off(|row| row.can_delete);
    let uninspectable = off(|row| row.inspection.is_some());
    let unexportable = off(|row| row.access.can_view() && exportable(row.block_type));
    let name = create_memo(clone!(row -> move || row.get().map(|row| row.label).unwrap_or_default()));
    let delete_label = create_memo(clone!(row -> move || {
        match row.get().is_some_and(|row| row.is_reference) {
            true => "Remove link".to_owned(),
            false => "Delete".to_owned(),
        }
    }));
    let running = set_acting.clone();
    let run = move |path: &'static [usize]| {
        let chosen = Rc::clone(&chosen);
        let set_acting = running.clone();
        move || {
            chosen(path.to_vec());
            set_acting.set(None);
        }
    };
    let host = editor.host().clone();
    let opening = clone!(row set_acting -> move || {
        let Some(shown) = row.get_untracked() else {
            return;
        };
        set_acting.set(None);
        if drills_in(&shown) {
            let Some(id) = shown.id else {
                return;
            };
            set_level.set(Level::Folder(vec![Crumb {
                id,
                block_type: shown.block_type,
                name: shown.label,
            }]));
            return;
        }
        let Some(id) = shown.id else {
            return;
        };
        match shown.container {
            Some(container) => host.open_block_via(id, shown.block_type, container),
            None => host.open_block(id, shown.block_type),
        }
    });
    let closing = set_acting.clone();
    view! {
        <ModalSheet open={open} fit=true on_close={move || closing.set(None)}>
            <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                <List spacing=0.0>
                    <Frame padding_horizontal=12.0 padding_vertical=PADDING>
                        <Heading content={name} />
                    </Frame>
                    <ActionRow
                        @test_id={"file-tree.actions.open"}
                        label="Open"
                        glyph={ICON_OPEN_IN_NEW.to_owned()}
                        disabled={unviewable}
                        on_click={opening}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.add"}
                        label="New file inside"
                        glyph={ICON_NOTE_ADD.to_owned()}
                        disabled={unaddable}
                        on_click={add_inside}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.rename"}
                        label="Rename"
                        glyph={ICON_DRIVE_FILE_RENAME_OUTLINE.to_owned()}
                        disabled={uneditable}
                        on_click={run(&[2])}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.share"}
                        label="Share"
                        glyph={ICON_SHARE.to_owned()}
                        disabled={unshareable}
                        on_click={run(&[3])}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.top"}
                        label="Move to the top level"
                        glyph={ICON_MOVE_UP.to_owned()}
                        disabled={unmovable}
                        on_click={run(&[1, 0])}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.unlink"}
                        label="Unlink"
                        glyph={ICON_LINK_OFF.to_owned()}
                        disabled={unlinkable}
                        on_click={run(&[4])}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.inspect"}
                        label="Inspect"
                        glyph={ICON_INFO.to_owned()}
                        disabled={uninspectable}
                        on_click={run(&[6])}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.export"}
                        label="Export"
                        glyph={ICON_FILE_DOWNLOAD.to_owned()}
                        disabled={unexportable}
                        on_click={run(&[7])}
                    />
                    <ActionRow
                        @test_id={"file-tree.actions.delete"}
                        label={delete_label}
                        glyph={ICON_DELETE.to_owned()}
                        disabled={undeletable}
                        danger=true
                        on_click={run(&[5])}
                    />
                </List>
            </Frame>
        </ModalSheet>
    }
}
