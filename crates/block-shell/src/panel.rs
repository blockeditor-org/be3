use std::cell::RefCell;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_LOCK;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::dock::unstyled::{TabId, dock_menu, use_dock_tab};
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{AccessLevel, BlockInfo, BlockList, BlockParent, BlockQuery, Blocks};
use block_editor_beui::{
    ArtifactState, BarAction, ChildBlock, ChildBlockHandle, ChildMode, ChildTarget, Editor, TopBar,
};
use uuid::Uuid;

use super::artifact::ArtifactBar;
use super::block_menu::with_block_menu;
use super::deleted::DeletedBar;
use super::linked::LinkedBar;
use super::phone::DetailsSheet;
use super::revisions::RevisionPreviewBar;
use super::tab::TabItem;
use super::workspace::{PanelStatus, PhoneSheet, Workspace};

const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 6.0;

#[derive(Clone, PartialEq)]
pub(crate) struct Refs {
    pub(crate) state: Ready<()>,
    pub(crate) list: Vec<BlockInfo>,
}

impl Default for Refs {
    fn default() -> Self {
        Self {
            state: Err(Pending::Loading),
            list: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Info {
    pub(crate) item: TabItem,
    pub(crate) editor: Option<Uuid>,
    pub(crate) view: Option<Uuid>,
    pub(crate) access: AccessLevel,
    pub(crate) ceiling: AccessLevel,
    pub(crate) can_edit: bool,
    pub(crate) dynamic_artifact: bool,
    pub(crate) type_name: String,
    pub(crate) parent: Option<BlockParent>,
    pub(crate) container: Option<Uuid>,
    pub(crate) parents: Refs,
    pub(crate) references: Refs,
    pub(crate) backrefs: Refs,
    pub(crate) artifact: Option<ArtifactState>,
    pub(crate) deleted: Option<Deleted>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Deleted {
    pub(crate) root: Uuid,
    pub(crate) inside: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    Block,
    Denied,
}

#[derive(Default)]
pub(crate) struct Watched {
    id: Option<Uuid>,
    parents: Option<BlockList>,
    references: Option<BlockList>,
    backrefs: Option<BlockList>,
}

impl Watched {
    fn follow(&mut self, client: &Blocks, id: Uuid) {
        if self.id == Some(id) {
            return;
        }
        self.id = Some(id);
        self.parents = Some(client.watch(BlockQuery::Parents(id)));
        self.references = Some(client.watch(BlockQuery::References(id)));
        self.backrefs = Some(client.watch(BlockQuery::Backrefs(id)));
    }
}

fn refs(list: Option<&BlockList>) -> Refs {
    list.map_or_else(Refs::default, |list| match list.read() {
        Ok(list) => Refs {
            state: Ok(()),
            list,
        },
        Err(pending) => Refs {
            state: Err(pending),
            list: Vec::new(),
        },
    })
}

pub(crate) fn read_info(
    workspace: &Workspace,
    tab: TabId,
    watched: &RefCell<Watched>,
) -> Option<Info> {
    let item = workspace.tab(tab)?;
    workspace.record_type(item.id, item.block_type);
    let mut watched = watched.borrow_mut();
    watched.follow(&workspace.blocks(), item.id);
    let access = workspace.access(item.id);
    let ceiling = workspace.ceiling(item.id);
    let own = workspace.info(item.id);
    let parent = own.as_ref().and_then(|info| info.parent);
    let parents = refs(watched.parents.as_ref());
    let references = refs(watched.references.as_ref());
    let backrefs = refs(watched.backrefs.as_ref());
    for reference in parents
        .list
        .iter()
        .chain(references.list.iter())
        .chain(backrefs.list.iter())
    {
        workspace.record_reference_types(reference);
    }
    let types = workspace.types();
    let deleted = own.filter(|info| info.deleted).map(|info| {
        let root = parents.list.iter().find(|above| above.deleted);
        Deleted {
            root: root.map_or(info.id, |root| root.id),
            inside: root.map(|root| root.label(types.as_ref()).name),
        }
    });
    let type_name = types
        .display_name(item.block_type)
        .map_or_else(|| item.block_type.to_string(), str::to_owned);
    Some(Info {
        editor: workspace.editor_of(&item),
        item,
        view: workspace.view_of(tab),
        access,
        ceiling,
        can_edit: workspace.can_edit(item.id),
        dynamic_artifact: workspace
            .info(item.id)
            .is_some_and(|info| info.is_artifact()),
        type_name,
        parent,
        container: workspace.container_of(item.id),
        parents,
        references,
        backrefs,
        artifact: {
            workspace.editor().artifacts().get();
            workspace.host().artifact(item.id)
        },
        deleted,
    })
}

#[component]
pub(crate) fn BlockPanel(workspace: Workspace, tab: TabId) -> NodeId {
    let info = create_signal(None::<Info>);
    let watched = RefCell::new(Watched::default());
    create_effect(move || {
        info.set(read_info(&workspace, tab, &watched));
    });
    let content = create_memo(move || match info.get() {
        Some(info) if !info.access.can_view() => Content::Denied,
        Some(_) | None => Content::Block,
    });
    let editor = workspace.editor();
    let phone = workspace.phone;
    let top_bar = create_memo(move || match phone.get() {
        true => TopBar::Phone,
        false => TopBar::Shown,
    });
    let session = untrack(|| workspace.tab(tab)).is_some_and(|item| {
        item.block_type
            == <workspace_ui_types::EditorViewContent as block_editor_beui::be_block::BlockContent>::CONTENT_TYPE
    });
    let preview = create_memo(move || workspace.revisions.preview_for(workspace.tab(tab)));
    let previewing = create_memo(move || preview.get().is_some());
    let live = create_memo(move || !previewing.get());
    let child_menu = create_signal(Vec::<MenuNode>::new());
    dock_menu(create_memo(move || {
        let nodes = child_menu.get();
        match session {
            true => nodes,
            false => info.with(|info| match info {
                Some(info) => with_block_menu(workspace, info, nodes),
                None => nodes,
            }),
        }
    }));
    let top = NodeRef::new();
    workspace.register_tab_node(tab, top);
    on_cleanup(move || workspace.forget_tab_node(tab, top));
    view! {
        <Frame @node_ref={&top}>
            <List spacing=0.0>
                <Show condition={!session}>
                    <ArtifactBar workspace={workspace} info={info.read_only()} />
                </Show>
                <Show condition={!session}>
                    <DeletedBar workspace={workspace} info={info.read_only()} />
                </Show>
                <Show condition={!session}>
                    <LinkedBar workspace={workspace} info={info.read_only()} />
                </Show>
                <RevisionPreviewBar history={workspace.revisions} tab={tab} workspace />
                <ShowKeepAlive condition={live}>
                    <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                        <Dynamic value={content}>|content: Content| {
                            match content {
                                Content::Denied => view! {
                                    <AccessDenied
                                        @sizing=ItemSize::Percent(100.0)
                                        info={info.read_only()}
                                    />
                                },
                                Content::Block => view! {
                                    <BlockChild
                                        @sizing=ItemSize::Percent(100.0)
                                        editor={editor}
                                        info={info.read_only()}
                                        menu={child_menu}
                                        top_bar={top_bar}
                                        on_bar={move |action: BarAction| match action {
                                            BarAction::Details => {
                                                workspace.set_sheet(PhoneSheet::Details(tab))
                                            }
                                        }}
                                    />
                                },
                            }
                        }</Dynamic>
                    </List>
                </ShowKeepAlive>
                <Show condition={previewing}>
                    <ChildBlock
                        @sizing=ItemSize::Percent(100.0)
                        editor
                        block={preview}
                        mode=ChildMode::Live
                        own_frame=true
                        top_bar=TopBar::Hidden
                        @test_id={"workspace.revision.preview"}
                    >|handle: ChildBlockHandle|
                        <PanelStatus state={handle.state} loading="Loading revision…" />
                    </ChildBlock>
                </Show>
                <DetailsSheet workspace={workspace} tab={tab} />
            </List>
        </Frame>
    }
}

#[component]
fn BlockChild(
    editor: Editor,
    info: ReadSignal<Option<Info>>,
    menu: Signal<Vec<MenuNode>>,
    top_bar: Memo<TopBar>,
    on_bar: block_editor_beui::beui::reactive::Callback<BarAction>,
) -> NodeId {
    let target = create_memo(move || {
        info.with(|info| {
            info.as_ref().map(|info| {
                let target =
                    ChildTarget::new(info.item.id, info.item.block_type).edited_with(info.editor);
                match info.view {
                    Some(view) => target.viewed_by(view),
                    None => target,
                }
            })
        })
    });
    let tab = use_dock_tab();
    view! {
        <ChildBlock
            editor={editor}
            block={target}
            mode=ChildMode::Live
            own_frame=true
            top_bar={top_bar}
            on_bar={on_bar}
            on_focus={move |()| tab.iter().for_each(|tab| tab.show())}
            @test_id={"workspace.block"}
        >|handle: ChildBlockHandle| {
            let own = handle.menu;
            create_effect(move || menu.set(own.get()));
            on_cleanup(move || menu.set(Vec::new()));
            view! {
                <PanelStatus state={handle.state} loading="This block is loading…" />
            }
        }</ChildBlock>
    }
}

#[component]
fn AccessDenied(info: ReadSignal<Option<Info>>) -> NodeId {
    let simulated = create_memo(move || {
        info.with(|info| info.as_ref().is_some_and(|info| info.ceiling.can_view()))
    });
    let first = create_memo(move || match simulated.get() {
        true => "An account that only knows this block exists cannot open it.".to_owned(),
        false => "You do not have permission to open this block.".to_owned(),
    });
    let second = create_memo(move || match simulated.get() {
        true => "Switch back to Can view or Can edit to see it.".to_owned(),
        false => "Ask someone who can edit it to share it with you.".to_owned(),
    });
    view! {
        <Frame padding=PANEL_PADDING>
            <List spacing=PANEL_SPACING align=Align::Center>
                <Text style=TextStyle::HEADING>
                    <Icon glyph=ICON_LOCK />
                    "No access"
                </Text>
                <Text style=TextStyle::CAPTION>{first}</Text>
                <Text style=TextStyle::CAPTION>{second}</Text>
            </List>
        </Frame>
    }
}
