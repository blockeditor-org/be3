use std::cell::RefCell;
use std::rc::Rc;

use block::{BlockAccess, BlockParent, BlockReference, BlockReferenceList};
use block_client::{BlockClient, ReferenceList};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::icons::ICON_LOCK;
use block_editor_plugin::beui::reactive::{
    Align, Dynamic, Frame, ItemSize, List, ReadSignal, clone, component, create_memo,
    create_signal, view,
};
use block_editor_plugin::beui::styled::{Caption, Heading, Separator};
use block_editor_plugin::beui::unstyled::TabId;
use block_editor_plugin::block_ui::BlockTypes;
use block_editor_plugin::{
    ArtifactState, ChildBlock, ChildBlockHandle, ChildMode, ChildTarget, Editor,
};
use uuid::Uuid;

use super::artifact::ArtifactBar;
use super::block_data::BlockData;
use super::chrome::ChromeBar;
use super::linked::LinkedBar;
use super::status::StatusBar;
use super::tab::TabItem;
use super::workspace::{PanelStatus, Workspace};

const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 6.0;
const SEPARATOR_HEIGHT: f32 = 1.0;

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Refs {
    pub(crate) loaded: bool,
    pub(crate) list: Vec<BlockReference>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Info {
    pub(crate) item: TabItem,
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
    pub(crate) access: BlockAccess,
    pub(crate) ceiling: BlockAccess,
    pub(crate) can_edit: bool,
    pub(crate) debugging: bool,
    pub(crate) dynamic_artifact: bool,
    pub(crate) type_name: String,
    pub(crate) label: String,
    pub(crate) glyph: String,
    pub(crate) automatic: bool,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) parent: Option<BlockParent>,
    pub(crate) container: Option<Uuid>,
    pub(crate) parents: Refs,
    pub(crate) references: Refs,
    pub(crate) backrefs: Refs,
    pub(crate) artifact: Option<ArtifactState>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Content {
    Block,
    Debug,
    Denied,
}

#[derive(Default)]
struct Watched {
    id: Option<Uuid>,
    parents: Option<ReferenceList>,
    references: Option<ReferenceList>,
    backrefs: Option<ReferenceList>,
}

impl Watched {
    fn follow(&mut self, client: &BlockClient, id: Uuid) {
        if self.id == Some(id) {
            return;
        }
        self.id = Some(id);
        self.parents = Some(client.watch_parents(id));
        self.references = Some(client.watch_references(BlockReferenceList::References(id)));
        self.backrefs = Some(client.watch_references(BlockReferenceList::Backrefs(id)));
    }
}

fn refs(list: Option<&ReferenceList>) -> Refs {
    list.map_or_else(Refs::default, |list| Refs {
        loaded: list.is_loaded(),
        list: list.read(),
    })
}

fn read_info(workspace: &Workspace, tab: TabId, watched: &RefCell<Watched>) -> Option<Info> {
    let block = workspace.tab(tab)?;
    let item = block.current();
    workspace.record_type(item.id, item.block_type);
    let mut watched = watched.borrow_mut();
    watched.follow(workspace.client(), item.id);
    let access = workspace.access(item.id);
    let ceiling = workspace.ceiling(item.id);
    let debugging = workspace.is_debugged(item.id);
    if debugging && !ceiling.can_view() {
        workspace.debug(item.id, false);
    }
    let history = if access.can_edit() {
        workspace.history(item)
    } else {
        (false, false)
    };
    let parent = workspace
        .read_handle(item, |handle| {
            handle
                .relationships()
                .map(|relationships| relationships.parent)
        })
        .flatten();
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
    let type_name = types
        .display_name(item.block_type)
        .map_or_else(|| item.block_type.to_string(), str::to_owned);
    let label = workspace.label(item.id, item.block_type);
    Some(Info {
        item,
        can_go_back: block.can_go_back(),
        can_go_forward: block.can_go_forward(),
        access,
        ceiling,
        can_edit: workspace.can_edit(item.id),
        debugging: debugging && ceiling.can_view(),
        dynamic_artifact: workspace.client().is_dynamic_artifact(item.id),
        type_name,
        glyph: label.icon.map(str::to_owned).unwrap_or_default(),
        automatic: label.automatic,
        label: label.name,
        can_undo: history.0,
        can_redo: history.1,
        parent,
        container: workspace.container_of(item.id),
        parents,
        references,
        backrefs,
        artifact: workspace.host().artifact(item.id),
    })
}

#[component]
pub(crate) fn BlockPanel(workspace: Rc<Workspace>, tab: TabId) -> NodeId {
    let (info, set_info) = create_signal(None::<Info>);
    let reading = Rc::downgrade(&workspace);
    let watched = RefCell::new(Watched::default());
    workspace.editor().each_frame(move || {
        let Some(workspace) = reading.upgrade() else {
            return;
        };
        set_info.set(read_info(&workspace, tab, &watched));
    });
    let content = create_memo(clone!(info -> move || match info.get() {
        Some(info) if info.debugging => Content::Debug,
        Some(info) if !info.access.can_view() => Content::Denied,
        Some(_) | None => Content::Block,
    }));
    let editor = workspace.editor().clone();
    let branch = Rc::clone(&workspace);
    let branch_info = info.clone();
    view! {
        <Frame>
            <List spacing=0.0>
                <ArtifactBar workspace={Rc::clone(&workspace)} tab={tab} info={info.clone()} />
                <LinkedBar workspace={Rc::clone(&workspace)} tab={tab} info={info.clone()} />
                <ChromeBar workspace={Rc::clone(&workspace)} tab={tab} info={info.clone()} />
                <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                <Dynamic value={content}>
                    {move |content: Content| {
                        let workspace = Rc::clone(&branch);
                        let info = branch_info.clone();
                        let editor = editor.clone();
                        match content {
                            Content::Debug => view! {
                                <BlockData
                                    @sizing=ItemSize::Percent(100.0)
                                    workspace={workspace}
                                    info={info}
                                />
                            },
                            Content::Denied => view! {
                                <AccessDenied @sizing=ItemSize::Percent(100.0) info={info} />
                            },
                            Content::Block => view! {
                                <BlockChild
                                    @sizing=ItemSize::Percent(100.0)
                                    editor={editor}
                                    info={info}
                                />
                            },
                        }
                    }}
                </Dynamic>
                <StatusBar workspace={workspace} tab={tab} info={info} />
            </List>
        </Frame>
    }
}

#[component]
fn BlockChild(editor: Editor, info: ReadSignal<Option<Info>>) -> NodeId {
    let target = create_memo(move || {
        info.with(|info| {
            info.as_ref()
                .map(|info| ChildTarget::new(info.item.id, info.item.block_type))
        })
    });
    view! {
        <ChildBlock
            editor={editor}
            block={target}
            mode=ChildMode::Live
            own_frame=true
            @test_id={"workspace.block"}
        >
            {move |handle: ChildBlockHandle| view! {
                <PanelStatus state={handle.state} loading="This block is loading…" />
            }}
        </ChildBlock>
    }
}

#[component]
fn AccessDenied(info: ReadSignal<Option<Info>>) -> NodeId {
    let simulated = create_memo(clone!(info -> move || {
        info.with(|info| info.as_ref().is_some_and(|info| info.ceiling.can_view()))
    }));
    let first = create_memo(clone!(simulated -> move || match simulated.get() {
        true => "An account that only knows this block exists cannot open it.".to_owned(),
        false => "You do not have permission to open this block.".to_owned(),
    }));
    let second = create_memo(clone!(simulated -> move || match simulated.get() {
        true => "Switch back to Can view or Can edit to see it.".to_owned(),
        false => "Ask someone who can edit it to share it with you.".to_owned(),
    }));
    let heading = format!("{ICON_LOCK} No access");
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=PANEL_SPACING align=Align::Center>
                <Heading content={heading} />
                <Caption content={first} />
                <Caption content={second} />
            </List>
        </Frame>
    }
}
