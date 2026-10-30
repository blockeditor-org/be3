use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::be_block::Home;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::block_ui::BlockCatalog;
use block_editor_beui::{
    BlockInfo, BlockList, BlockParent, BlockQuery, ChildState, DialogAnswer, Editor, EditorHost,
};
use uuid::Uuid;

use super::picker::{Graph, Pick, PickAction, PickOutcome, PickTree, Tree, TreeRow};

struct State {
    handles: RefCell<HashMap<Uuid, BlockList>>,
    roots: RefCell<Option<(BlockList, BlockList)>>,
    trees: RefCell<HashMap<PickTree, HashMap<BlockQuery, BlockList>>>,
    next_ask: Cell<u64>,
}

#[derive(Clone, Copy)]
pub struct Picker {
    state: Stored<State>,
    editor: Editor,
    held: Signal<Option<Pick>>,
    pub(crate) sheet: bool,
}

impl Picker {
    pub fn new(editor: Editor) -> Option<Self> {
        let request = editor.host().picker()?;
        let sheet = request.sheet;
        Some(Self {
            state: store(State {
                handles: RefCell::new(HashMap::new()),
                roots: RefCell::new(None),
                trees: RefCell::new(HashMap::new()),
                next_ask: Cell::new(0),
            }),
            editor,
            held: create_signal(Some(Pick::new(request))),
            sheet,
        })
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn host(&self) -> EditorHost {
        self.editor.host()
    }

    pub(crate) fn pick(&self) -> Option<Pick> {
        self.held.get()
    }

    pub(crate) fn held(&self) -> Memo<Option<Pick>> {
        let held = self.held;
        create_memo(move || held.get())
    }

    pub(crate) fn types(&self) -> Rc<BlockCatalog> {
        self.editor.block_types()
    }

    pub(crate) fn info(&self, id: Uuid) -> Option<BlockInfo> {
        let blocks = self.editor.blocks();
        let inner = self.state.value();
        let mut handles = inner.handles.borrow_mut();
        let list = handles
            .entry(id)
            .or_insert_with(|| blocks.watch(BlockQuery::Block(id)));
        list.read()
            .ok()
            .and_then(|list| list.into_iter().next())
            .or_else(|| blocks.info(id))
    }

    fn top(&self) -> (Vec<BlockInfo>, Vec<BlockInfo>) {
        let blocks = self.editor.blocks();
        let inner = self.state.value();
        let mut held = inner.roots.borrow_mut();
        let (roots, shared) = held.get_or_insert_with(|| {
            (
                blocks.watch(BlockQuery::Roots),
                blocks.watch(BlockQuery::Shared),
            )
        });
        (
            roots.read().unwrap_or_default(),
            shared.read().unwrap_or_default(),
        )
    }

    fn my_files(&self) -> Option<Uuid> {
        let host = self.host();
        let mine = block_editor_beui::be_block::my_files(host.organization_id(), host.account_id());
        self.top()
            .0
            .into_iter()
            .any(|block| block.id == mine && block_editor_beui::sections::is_workspace(&block))
            .then_some(mine)
    }

    pub(crate) fn templates_for(&self, parent: BlockParent) -> Vec<Uuid> {
        let Some(block) = parent.block().or_else(|| self.my_files()) else {
            return Vec::new();
        };
        self.editor
            .resolved_settings::<Home>(block)
            .with(|home| home.value().templates())
    }

    fn graph(&self, tree: PickTree) -> Graph {
        let (roots, shared) = self.top();
        let inner = self.state.value();
        let trees = inner.trees.borrow();
        let lists = trees.get(&tree);
        let mut blocks: HashMap<Uuid, BlockInfo> = HashMap::new();
        for block in lists
            .into_iter()
            .flat_map(HashMap::values)
            .flat_map(|list| list.read().unwrap_or_default())
            .chain(roots.iter().cloned())
            .chain(shared.iter().cloned())
        {
            blocks.insert(block.id, block);
        }
        let listed = lists
            .into_iter()
            .flat_map(HashMap::iter)
            .filter_map(|(query, list)| match query {
                BlockQuery::Children(id) if list.read().is_ok() => Some(*id),
                _ => None,
            })
            .collect();
        Graph {
            organization: self.host().organization_id(),
            account: self.host().account_id(),
            roots,
            shared,
            blocks,
            listed,
        }
    }

    fn hold(&self, tree: PickTree, wants: &[BlockQuery]) -> bool {
        let blocks = self.editor.blocks();
        let inner = self.state.value();
        let mut trees = inner.trees.borrow_mut();
        let lists = trees.entry(tree).or_default();
        let before = lists.len();
        lists.retain(|query, _| wants.contains(query));
        let mut changed = lists.len() != before;
        for query in wants {
            if !lists.contains_key(query) {
                lists.insert(*query, blocks.watch(*query));
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn tree(&self, tree: PickTree, rows: impl Fn(&Graph) -> Tree) -> Vec<TreeRow> {
        let mut built = rows(&self.graph(tree));
        for _ in 0..8 {
            if !self.hold(tree, &built.wants) {
                break;
            }
            built = rows(&self.graph(tree));
        }
        built.rows
    }

    fn ask_number(&self) -> u64 {
        let state = self.state.value();
        state.next_ask.set(state.next_ask.get() + 1);
        state.next_ask.get()
    }

    pub(crate) fn act(&self, action: PickAction) {
        let Some(mut held) = self.held.get_untracked() else {
            return;
        };
        if let PickAction::Import | PickAction::CopyTemplate(_) = action {
            if held.importing.is_none() {
                let ask = self.ask_number();
                let picker = *self;
                let parent = held.import_parent();
                let asked = match action {
                    PickAction::CopyTemplate(template) => self.host().copy_blocks(template, parent),
                    _ => self.host().import_blocks(parent),
                };
                self.editor.then(asked, move |imported| {
                    if let Some(held) = picker.held.get_untracked()
                        && held.importing == Some(ask)
                    {
                        picker.resolve(held.imported(imported));
                    }
                });
                held.importing = Some(ask);
                self.resolve(PickOutcome::Open(Box::new(held)));
            }
            return;
        }
        match held.act(action) {
            PickOutcome::Open(mut held) => {
                self.send_commit(&mut held);
                self.resolve(PickOutcome::Open(held));
            }
            answered => self.resolve(answered),
        }
    }

    fn send_commit(&self, held: &mut Pick) {
        let parent = held.created_parent();
        if let Some(creating) = &mut held.creating
            && let Some(child) = creating.child
            && creating.committed
            && !creating.sent
        {
            creating.sent = true;
            self.host().commit_child(child, parent, None);
        }
    }

    pub(crate) fn creation_state(&self, state: &ChildState) {
        let Some(mut held) = self.held.get_untracked() else {
            return;
        };
        if let Some(creating) = &mut held.creating {
            creating.child = state.child;
            creating.height = state.intrinsic_size.map(|size| size.y.max(1.0));
        }
        let held = match state.dialog.answer {
            Some(DialogAnswer::Dismissed) => {
                return self.act(PickAction::CancelCreation);
            }
            Some(DialogAnswer::Accepted) => match held.act(PickAction::Create) {
                PickOutcome::Open(held) => *held,
                answered => return self.resolve(answered),
            },
            None => held,
        };
        let mut held = held;
        self.send_commit(&mut held);
        match &state.creation {
            Some(progress) => self.resolve(held.progressed(progress)),
            None => self.resolve(PickOutcome::Open(Box::new(held))),
        }
    }

    fn resolve(&self, outcome: PickOutcome) {
        match outcome {
            PickOutcome::Open(held) => {
                if self.held.get_untracked().as_ref() != Some(&*held) {
                    self.held.set(Some(*held));
                }
            }
            PickOutcome::Answered(answer, into) => {
                self.held.set(None);
                self.state.value().trees.borrow_mut().clear();
                if let (Ok(Some(_)), Some((block, block_type, container))) = (&answer, into) {
                    self.host().place_block(block, block_type, container, false);
                }
                self.host().answer_pick(answer);
            }
        }
    }
}
