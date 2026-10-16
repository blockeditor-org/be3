use std::collections::{HashMap, HashSet};

use block_editor_beui::be_block::profile::RECENTS;
use block_editor_beui::be_block::{BlockContent, EditorViewContent, FolderContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_WORKSPACES;
use block_editor_beui::beui::icons::{
    ICON_ACCOUNT_CIRCLE, ICON_ADD, ICON_ARROW_BACK, ICON_CHEVRON_RIGHT, ICON_DELETE,
    ICON_DRIVE_FILE_RENAME_OUTLINE, ICON_HISTORY, ICON_INFO, ICON_LINK_OFF, ICON_LOGOUT,
    ICON_MORE_VERT, ICON_MOVE_UP, ICON_NOTE_ADD, ICON_OPEN_IN_NEW, ICON_PEOPLE, ICON_SEARCH,
    ICON_SEARCH_OFF, ICON_SETTINGS, ICON_SHARE,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{BackSlide, Edge, Floating, MenuItem};
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{BlockQuery, ChildTarget, Editor, watch_block_label};
use uuid::Uuid;

use super::export::Exporter;
use super::rows::{
    Inspection, Placement, Row, RowKey, Tree as FileTree, describe, is_workspace, own_settings,
    shared_with_me, workspaces,
};
use super::ui::{ExportAs, OpenWith, Picker, leavable, menu_action, new_workspace};
use block_editor_beui::beui::TextAlign;

const PADDING: f32 = 8.0;
const ROW_SPACING: f32 = 2.0;
const TILE_SIDE: f32 = 40.0;
const ROW_INNER_SPACING: f32 = 14.0;
const RECENT_WIDTH: f32 = 132.0;
const RECENT_HEIGHT: f32 = 96.0;
const RECENT_SHOWN: usize = 10;
const DOCK_ROOM: f32 = 88.0;
const NEW_MARGIN: f32 = 16.0;

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
    Shared,
    Deleted,
    Settings,
}

impl Level {
    fn query(&self) -> BlockQuery {
        match self {
            Self::Root => BlockQuery::Roots,
            Self::Folder(crumbs) => crumbs
                .last()
                .map_or(BlockQuery::Roots, |crumb| BlockQuery::References(crumb.id)),
            Self::Shared => BlockQuery::Shared,
            Self::Deleted => BlockQuery::Detached,
            Self::Settings => BlockQuery::Roots,
        }
    }

    fn container(&self) -> Option<&Crumb> {
        match self {
            Self::Folder(crumbs) => crumbs.last(),
            Self::Root | Self::Shared | Self::Deleted | Self::Settings => None,
        }
    }

    fn up(&self) -> Self {
        match self {
            Self::Folder(crumbs) if crumbs.len() > 1 => {
                Self::Folder(crumbs[..crumbs.len() - 1].to_vec())
            }
            Self::Root | Self::Folder(_) | Self::Shared | Self::Deleted | Self::Settings => {
                Self::Root
            }
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
            Self::Root | Self::Shared | Self::Deleted | Self::Settings => Self::Folder(vec![crumb]),
        })
    }
}

fn drills_in(row: &Row) -> bool {
    (row.block_type == FolderContent::CONTENT_TYPE || is_workspace(row.block_type))
        && !row.is_reference
}

#[component]
pub(crate) fn PhoneFiles(
    editor: Editor,
    tree: FileTree,
    picker: Picker,
    exporter: Exporter,
    inspect: WriteSignal<Option<Inspection>>,
    covered: Memo<bool>,
) -> NodeId {
    let (level, set_level) = create_signal(Level::Root);
    let (query, set_query) = create_signal(String::new());
    let searching = create_memo(move || !query.get().trim().is_empty());
    let browsing = create_memo(move || !searching.get());
    let at_root = create_memo(move || browsing.get() && level.get() == Level::Root);
    let nested = create_memo(move || level.get() != Level::Root);
    let can_create = create_memo(move || {
        browsing.get()
            && !matches!(
                level.get(),
                Level::Deleted | Level::Shared | Level::Settings
            )
            && !covered.get()
    });
    let page = NodeRef::new();
    let creating = move || {
        let parent = level.with_untracked(|level| {
            level
                .container()
                .filter(|crumb| {
                    editor.block_types().child_edits(crumb.block_type).add
                        && tree.blocks().access(crumb.id).can_edit()
                })
                .map(|crumb| crumb.id)
                .or_else(|| match level {
                    Level::Root => tree.my_files(),
                    Level::Folder(_) | Level::Shared | Level::Deleted | Level::Settings => None,
                })
        });
        let Some(parent) = parent else {
            return;
        };
        let parent = Some(parent);
        let excluded = parent.into_iter().collect::<HashSet<Uuid>>();
        picker.open_placed(editor, parent, excluded);
    };
    let rising = move || set_level.set(level.get_untracked().up());
    let menu = move || editor.host().show_app_menu(editor.block_id());
    let shelf = Shelf {
        editor,
        tree,
        set_level,
        tools: RowTools {
            editor,
            tree,
            picker,
            exporter,
            inspect,
            set_level,
        },
    };
    let header_level = level;
    let header_set = set_level;
    let page_color = use_theme().background;
    let parent = move || {
        let (above, _) = create_signal(level.get_untracked().up());
        let (unsearched, unsearch) = create_signal(String::new());
        let at_top = create_memo(move || above.get() == Level::Root);
        let (color, folder, set_level, on_menu) = (page_color, shelf, shelf.set_level, menu);
        view! {
            <Frame fill={ColorRole::custom(color)}>
                <List spacing=0.0>
                    <Header
                        level={above}
                        set_level
                        query={unsearched}
                        set_query={unsearch}
                        on_menu
                    />
                    <Folder
                        @sizing=ItemSize::Percent(100.0)
                        shelf={folder}
                        level={above}
                        at_root={at_top}
                    />
                </List>
            </Frame>
        }
    };
    view! {
        <BackSlide enabled={nested} on_back={rising} behind={parent}>
            <Frame fill={ColorRole::custom(page_color)}>
                <List spacing=0.0>
                    <Header
                        level={header_level}
                        set_level={header_set}
                        query
                        set_query
                        on_menu={menu}
                    />
                    <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&page}>
                        <List spacing=0.0>
                            <Show condition={searching}>
                                <SearchSoon @sizing=ItemSize::Percent(100.0) />
                            </Show>
                            <Show condition={browsing}>
                                {move || view! {
                                    <Folder @sizing=ItemSize::Percent(100.0) shelf level at_root />
                                }}
                            </Show>
                        </List>
                    </Frame>
                    <Floating anchor={page} edge=Edge::BottomEnd open={can_create}>
                        <Frame padding=NEW_MARGIN>
                            <Button
                                @test_id={"file-tree.new"}
                                label="New"
                                glyph={ICON_ADD.to_owned()}
                                variant=ButtonVariant::Primary
                                on_click={creating}
                            />
                        </Frame>
                    </Floating>
                </List>
            </Frame>
        </BackSlide>
    }
}

#[derive(Clone, Copy)]
struct Shelf {
    editor: Editor,
    tree: FileTree,
    set_level: WriteSignal<Level>,
    tools: RowTools,
}

#[component]
fn Folder(shelf: Shelf, level: ReadSignal<Level>, at_root: Memo<bool>) -> NodeId {
    let recents_editor = shelf.editor;
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=ROW_SPACING>
                    <Show condition={at_root}>
                        <Recents editor={recents_editor} />
                    </Show>
                    <Dynamic value={level}>
                        {move |level: Level| {
                            let editor = shelf.editor;
                            let tree = shelf.tree;
                            let set_level = shelf.set_level;
                            let tools = shelf.tools;
                            view! {
                                <LevelRows
                                    @sizing=ItemSize::Intrinsic
                                    editor
                                    tree
                                    level
                                    set_level
                                    tools
                                />
                            }
                        }}
                    </Dynamic>
                    <Frame height=DOCK_ROOM />
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn Header(
    level: ReadSignal<Level>,
    set_level: WriteSignal<Level>,
    query: ReadSignal<String>,
    set_query: WriteSignal<String>,
    on_menu: ClickCallback,
) -> NodeId {
    let top = create_memo(move || level.get() == Level::Root);
    let inside = create_memo(move || !top.get());
    let titled = top;
    let named = inside;
    let name = create_memo(move || match level.get() {
        Level::Root => "Files".to_owned(),
        Level::Folder(crumbs) => crumbs
            .last()
            .map(|crumb| crumb.name.clone())
            .unwrap_or_default(),
        Level::Shared => "Shared with me".to_owned(),
        Level::Deleted => "Recently deleted".to_owned(),
        Level::Settings => "Settings".to_owned(),
    });
    let path = create_memo(move || match level.get() {
        Level::Folder(crumbs) => std::iter::once("Files".to_owned())
            .chain(
                crumbs[..crumbs.len().saturating_sub(1)]
                    .iter()
                    .map(|crumb| crumb.name.clone()),
            )
            .collect::<Vec<_>>()
            .join(" / "),
        Level::Root | Level::Shared | Level::Deleted | Level::Settings => "Files".to_owned(),
    });
    let up = move || set_level.set(level.get_untracked().up());
    let typed = query;
    view! {
        <Frame fill=ColorRole::Background padding=PADDING>
            <List spacing=PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                    <Show condition={inside}>
                        <IconButton
                            @test_id={"file-tree.back"}
                            glyph={ICON_ARROW_BACK.to_owned()}
                            label="Up one folder"
                            on_click={up}
                        />
                    </Show>
                    <Show condition={titled}>
                        <Frame
                            @sizing=ItemSize::Percent(100.0)
                            padding={Sides::symmetric(PADDING, 4.0)}
                        >
                            <Text style=TextStyle::TITLE string="Files" />
                        </Frame>
                    </Show>
                    <Show condition={named}>
                        {move || view! {
                            <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                                <Text style=TextStyle::CAPTION string={path} ellipsis=true />
                                <Text style=TextStyle::HEADING string={name} />
                            </List>
                        }}
                    </Show>
                    <Show condition={top}>
                        {move || view! {
                            <IconButton
                                @test_id={"file-tree.app-menu"}
                                glyph={ICON_ACCOUNT_CIRCLE.to_owned()}
                                label="Account and settings"
                                on_click={on_menu}
                            />
                        }}
                    </Show>
                </List>
                <TextInput
                    @test_id={"file-tree.search"}
                    value={typed}
                    placeholder="Search files"
                    label="Search files"
                    glyph={ICON_SEARCH.to_owned()}
                    clearable=true
                    on_change={move |value: String| set_query.set(value)}
                />
            </List>
        </Frame>
    }
}

#[component]
fn SearchSoon() -> NodeId {
    view! {
        <Frame padding={Sides::symmetric(32.0, 48.0)} @test_id={"file-tree.search.todo"}>
            <List spacing=10.0 align=Align::Center>
                <Text size=TextSize::Xl3 color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph={ICON_SEARCH_OFF.to_owned()} />
                </Text>
                <Text style=TextStyle::HEADING string="Search is coming soon" />
                <Text
                    style=TextStyle::CAPTION
                    string="It isn't built yet. Open folders to find a file for now."
                    wrap=true
                />
            </List>
        </Frame>
    }
}

#[component]
fn Recents(editor: Editor) -> NodeId {
    let view = editor
        .view_block()
        .map(|view| editor.watch_blocks(BlockQuery::Block(view)));
    let profile = create_memo(move || {
        let listed = view.as_ref()?.get()?;
        listed.first()?.parent?.block()
    });
    let own = editor.block_id();
    let recents = editor
        .related_content::<EditorViewContent>(profile)
        .project(move |content| {
            content
                .root()
                .state(RECENTS)
                .and_then(|state| state.value::<block_editor_beui::be_block::Recents>())
                .unwrap_or_default()
                .0
                .into_iter()
                .map(|recent| (recent.block, recent.block_type))
                .filter(|(id, _)| *id != own)
                .take(RECENT_SHOWN)
                .collect::<Vec<(Uuid, Uuid)>>()
        });
    let keys = create_memo(move || {
        recents.with(|recents| recents.iter().map(|(id, _)| *id).collect::<Vec<Uuid>>())
    });
    let any = create_memo(move || !keys.get().is_empty());
    view! {
        <List spacing=0.0>
            <Show condition={any}>
                {move || view! {
                    <List spacing=6.0>
                        <Frame padding={Sides::symmetric(PADDING, 4.0)}>
                            <Text style=TextStyle::CAPTION string="Recent" />
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
                                    view! {
                                        <RecentCard editor id block_type />
                                    }
                                }}
                            </ForEach>
                        </Scroll>
                        <Frame padding={Sides::symmetric(PADDING, 4.0)}>
                            <Text style=TextStyle::CAPTION string="All files" />
                        </Frame>
                    </List>
                }}
            </Show>
        </List>
    }
}

#[component]
fn RecentCard(editor: Editor, id: Uuid, block_type: Uuid) -> NodeId {
    let target = create_memo(move || Some(ChildTarget::new(id, block_type)));
    let shown = watch_block_label(editor, target);
    let resolved = create_memo(move || shown.with(|shown| shown.resolved));
    let name = create_memo(move || shown.with(|shown| shown.name.clone()));
    let glyph = create_memo(move || shown.with(|shown| shown.glyph.clone()));
    let kind = create_memo(move || shown.with(|shown| shown.type_name.clone()));
    let host = editor.host();
    view! {
        <Frame visible={resolved} padding={Sides::horizontal(4.0)}>
            <Frame
                width=RECENT_WIDTH
                height=RECENT_HEIGHT
                radius=Radius::Lg
                fill=ColorRole::SurfaceRaised
                border={Border::new(ColorRole::Border).width(1.0)}
            >
                <ListRow
                    @test_id={format!("file-tree.recent.{id}")}
                    on_click={move || host.open_block(id, block_type)}
                >
                    <Frame height={RECENT_HEIGHT - 8.0} padding={Sides::vertical(6.0)}>
                        <List spacing=6.0>
                            <List direction=Direction::Horizontal spacing=0.0>
                                <Text color=TextColor::Accent align=TextAlign::Center>
                                    <Icon glyph={glyph} />
                                </Text>
                            </List>
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Text string={name} ellipsis=true />
                            <Text style=TextStyle::CAPTION string={kind} ellipsis=true />
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
    tree: FileTree,
    level: Level,
    set_level: WriteSignal<Level>,
    tools: RowTools,
) -> NodeId {
    let listed = editor.watch_blocks(level.query());
    let roots = match level {
        Level::Shared => Some(editor.watch_blocks(BlockQuery::Roots)),
        Level::Root | Level::Folder(_) | Level::Deleted | Level::Settings => None,
    };
    let described_level = level.clone();
    let account = editor.host().account_id();
    let rows = create_memo(move || {
        let listed = listed.get()?;
        let types = editor.block_types();
        let listed = match &described_level {
            Level::Root => workspaces(listed, account, types.as_ref()),
            Level::Shared => shared_with_me(roots.as_ref()?.get()?, listed),
            Level::Settings => own_settings(listed, account),
            Level::Folder(_) | Level::Deleted => listed,
        };
        let mut block_types: HashMap<Uuid, Uuid> = tree.block_types();
        if let Level::Folder(crumbs) = &described_level {
            block_types.extend(crumbs.iter().map(|crumb| (crumb.id, crumb.block_type)));
        }
        block_types.extend(listed.iter().map(|info| (info.id, info.block_type)));
        let path: Vec<Uuid> = match &described_level {
            Level::Folder(crumbs) => crumbs.iter().map(|crumb| crumb.id).collect(),
            Level::Root | Level::Shared | Level::Deleted | Level::Settings => Vec::new(),
        };
        let container = described_level.container().map(|crumb| crumb.id);
        let entry = matches!(described_level, Level::Root | Level::Shared);
        Some(
            listed
                .iter()
                .map(|info| {
                    let mut key = path.clone();
                    key.push(info.id);
                    let mut row = describe(
                        &tree.blocks(),
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
                    );
                    row.can_leave = entry && info.author != Some(account);
                    row
                })
                .collect::<Vec<Row>>(),
        )
    });
    let loading = create_memo(move || rows.with(Option::is_none));
    let empty = create_memo(move || rows.with(|rows| rows.as_ref().is_some_and(Vec::is_empty)));
    let keys = create_memo(move || {
        rows.with(|rows| {
            rows.iter()
                .flatten()
                .filter_map(|row| row.id)
                .collect::<Vec<Uuid>>()
        })
    });
    let empty_text = match &level {
        Level::Root => "No workspaces yet. Tap New workspace to make one.",
        Level::Folder(_) => "This folder is empty. Tap New to add a file to it.",
        Level::Shared => "Nothing has been shared with you.",
        Level::Deleted => "Nothing has been deleted.",
        Level::Settings => "No settings yet.",
    };
    let at_root = level == Level::Root;
    let deleting = set_level;
    view! {
        <List spacing=ROW_SPACING>
            <Show condition={loading}>
                <Frame padding=PADDING>
                    <Text style=TextStyle::CAPTION string="Loading…" />
                </Frame>
            </Show>
            <Show condition={empty}>
                <Frame padding={Sides::symmetric(PADDING, 24.0)} @test_id={"file-tree.empty"}>
                    <Text style=TextStyle::CAPTION string={empty_text} wrap=true />
                </Frame>
            </Show>
            <ForEach keys={keys}>
                {move |id: Uuid| {
                    let row = create_memo(move || {
                        rows.with(|rows| {
                            rows.iter().flatten().find(|row| row.id == Some(id)).cloned()
                        })
                    });
                    let level = level.clone();
                    let set_level = set_level;
                    let tools = tools;
                    view! {
                        <PhoneRow editor row level set_level tools />
                    }
                }}
            </ForEach>
            <Show condition={at_root}>
                {move || view! {
                    <ActionRow
                        @test_id={"file-tree.new-workspace"}
                        label="New workspace"
                        glyph={ICON_WORKSPACES.to_owned()}
                        on_click={move || new_workspace(editor)}
                    />
                }}
            </Show>
            <Show condition={at_root}>
                {move || view! {
                    <ActionRow
                        @test_id={"file-tree.shared"}
                        label="Shared with me"
                        glyph={ICON_PEOPLE.to_owned()}
                        on_click={move || deleting.set(Level::Shared)}
                    />
                }}
            </Show>
            <Show condition={at_root}>
                {move || view! {
                    <ActionRow
                        @test_id={"file-tree.deleted"}
                        label="Recently deleted"
                        glyph={ICON_HISTORY.to_owned()}
                        on_click={move || deleting.set(Level::Deleted)}
                    />
                }}
            </Show>
            <Show condition={at_root}>
                {move || view! {
                    <ActionRow
                        @test_id={"file-tree.settings"}
                        label="Settings"
                        glyph={ICON_SETTINGS.to_owned()}
                        on_click={move || deleting.set(Level::Settings)}
                    />
                }}
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
    tools: RowTools,
) -> NodeId {
    let id = row
        .get_untracked()
        .and_then(|row| row.id)
        .map(|id| id.to_string())
        .unwrap_or_default();
    let open_id = format!("file-tree.{id}.open");
    let inside_id = format!("file-tree.{id}.inside");
    let more_id = format!("file-tree.{id}.more");
    let name = create_memo(move || row.get().map(|row| row.label).unwrap_or_default());
    let glyph = create_memo(move || row.get().map(|row| row.glyph).unwrap_or_default());
    let detail = create_memo(move || {
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
            false => editor
                .block_types()
                .display_name(row.block_type)
                .unwrap_or("Block")
                .to_owned(),
        }
    });
    let muted = create_memo(move || row.get().is_none_or(|row| row.automatic));
    let color = create_memo(move || match muted.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let folder = create_memo(move || row.get().is_some_and(|row| drills_in(&row)));
    let holds = create_memo(move || {
        row.get()
            .is_some_and(|row| row.expandable && !drills_in(&row))
    });
    let entering = move || {
        if let Some(next) = row.get_untracked().and_then(|row| level.inside(&row)) {
            set_level.set(next);
        }
    };
    let tapped = {
        let entering = entering.clone();
        move || {
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
                Some(container) => editor
                    .host()
                    .open_block_via(id, shown.block_type, container),
                None => editor.host().open_block(id, shown.block_type),
            }
        }
    };
    let muted_icon = TextColor::Muted;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
            <ListRow @sizing=ItemSize::Percent(100.0) @test_id={open_id} on_click={tapped}>
                <Frame padding={Sides::vertical(6.0)}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=ROW_INNER_SPACING
                    >
                        <Frame
                            width=TILE_SIDE
                            height=TILE_SIDE
                            radius=Radius::Md
                            fill=ColorRole::SurfaceRaised
                            align=Align::Center
                        >
                            <Text size=TextSize::Lg color=TextColor::Accent align=TextAlign::Center>
                                <Icon glyph={glyph} />
                            </Text>
                        </Frame>
                        <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                            <Text string={name} color={color} ellipsis=true />
                            <Text style=TextStyle::CAPTION string={detail} ellipsis=true />
                        </List>
                        <Show condition={folder}>
                            <Text color={muted_icon} align=TextAlign::Center>
                                <Icon glyph={ICON_CHEVRON_RIGHT.to_owned()} />
                            </Text>
                        </Show>
                    </List>
                </Frame>
            </ListRow>
            <Show condition={holds}>
                <IconButton
                    @test_id={inside_id.clone()}
                    glyph={ICON_CHEVRON_RIGHT.to_owned()}
                    label="Show what it holds"
                    on_click={entering.clone()}
                />
            </Show>
            <RowMenu @test_id={more_id} tools row={row} />
        </List>
    }
}

#[derive(Clone, Copy)]
struct RowTools {
    editor: Editor,
    tree: FileTree,
    picker: Picker,
    exporter: Exporter,
    inspect: WriteSignal<Option<Inspection>>,
    set_level: WriteSignal<Level>,
}

#[component]
fn RowMenu(tools: RowTools, row: Memo<Option<Row>>) -> NodeId {
    let RowTools {
        editor,
        tree,
        picker,
        exporter,
        inspect,
        set_level,
    } = tools;
    let chosen = menu_action(editor, picker, inspect, row);
    let my_files = create_memo(move || {
        tree.rows().get();
        tree.my_files()
    });
    let moving = move || {
        let (Some(shown), Some(files)) = (row.get_untracked(), my_files.get_untracked()) else {
            return;
        };
        let Some(id) = shown.id else {
            return;
        };
        editor.host().move_block(
            id,
            shown.block_type,
            shown.source,
            files,
            shown.is_reference,
        );
    };
    let add_inside = move || {
        let Some(id) = row.get_untracked().and_then(|row| row.id) else {
            return;
        };
        picker.open_placed(editor, Some(id), [id].into_iter().collect());
    };
    let off =
        |test: fn(&Row) -> bool| create_memo(move || !row.get().is_some_and(|row| test(&row)));
    let unviewable = off(|row| row.access.can_view());
    let unaddable = off(|row| row.can_add);
    let uneditable = off(|row| row.can_edit);
    let unshareable = off(|row| row.can_edit);
    let unmovable = create_memo(move || {
        let files = my_files.get();
        !row.get().is_some_and(|row| {
            row.can_delete && files.is_some() && row.container != files && row.id != files
        })
    });
    let unlinkable = off(|row| row.is_reference && row.unlink.is_ok());
    let undeletable = off(|row| row.can_delete);
    let can_leave = leavable(editor, row);
    let leave = move || {
        if let Some(id) = row.get_untracked().and_then(|row| row.id) {
            editor.host().leave_block(id);
        }
    };
    let uninspectable = off(|row| row.inspection.is_some());
    let delete_label = create_memo(
        move || match row.get().is_some_and(|row| row.is_reference) {
            true => "Remove link".to_owned(),
            false => "Delete".to_owned(),
        },
    );
    let opening = move || {
        let Some(shown) = row.get_untracked() else {
            return;
        };
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
            Some(container) => editor
                .host()
                .open_block_via(id, shown.block_type, container),
            None => editor.host().open_block(id, shown.block_type),
        }
    };
    view! {
        <IconMenuButton
            variant=ButtonVariant::Secondary
            glyph={ICON_MORE_VERT.to_owned()}
            label="More actions"
            items={view! {
                <MenuItem
                    row_test_id={"file-tree.actions.open".to_owned()}
                    label="Open"
                    glyph={ICON_OPEN_IN_NEW.to_owned()}
                    disabled={unviewable}
                />
                <OpenWith editor={editor} row={row} />
                <MenuItem
                    row_test_id={"file-tree.actions.add".to_owned()}
                    label="New file inside"
                    glyph={ICON_NOTE_ADD.to_owned()}
                    disabled={unaddable}
                />
                <MenuItem
                    row_test_id={"file-tree.actions.rename".to_owned()}
                    label="Rename"
                    glyph={ICON_DRIVE_FILE_RENAME_OUTLINE.to_owned()}
                    disabled={uneditable}
                />
                <MenuItem
                    row_test_id={"file-tree.actions.share".to_owned()}
                    label="Share"
                    glyph={ICON_SHARE.to_owned()}
                    disabled={unshareable}
                />
                <MenuItem
                    row_test_id={"file-tree.actions.top".to_owned()}
                    label="Move to My files"
                    glyph={ICON_MOVE_UP.to_owned()}
                    disabled={unmovable}
                />
                <MenuItem
                    row_test_id={"file-tree.actions.unlink".to_owned()}
                    label="Unlink"
                    glyph={ICON_LINK_OFF.to_owned()}
                    disabled={unlinkable}
                />
                <MenuItem
                    row_test_id={"file-tree.actions.inspect".to_owned()}
                    label="Inspect"
                    glyph={ICON_INFO.to_owned()}
                    disabled={uninspectable}
                />
                <ExportAs
                    exporter={exporter}
                    row={row}
                    row_test_id={"file-tree.actions.export".to_owned()}
                />
                <MenuItem
                    row_test_id={"file-tree.actions.delete".to_owned()}
                    label={delete_label}
                    glyph={ICON_DELETE.to_owned()}
                    disabled={undeletable}
                    danger=true
                    separated=true
                />
                <Show condition={can_leave}>
                    <MenuItem
                        row_test_id={"file-tree.actions.leave".to_owned()}
                        label="Leave"
                        glyph={ICON_LOGOUT.to_owned()}
                        danger=true
                        on_click={leave}
                    />
                </Show>
            }}
            on_select={move |path: Vec<usize>| match path.as_slice() {
                [0] => opening(),
                [2] => add_inside(),
                [3] => chosen(vec![3]),
                [4] => chosen(vec![4]),
                [5] => moving(),
                [6] => chosen(vec![5]),
                [7] => chosen(vec![7]),
                [9] => chosen(vec![6]),
                _ => {}
            }}
        />
    }
}
