use block_editor_beui::be_block::BlockContent;
use logic_grid_types::{Hotbar, HotbarContent};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::root_settings::RootSetting;
use block_editor_beui::{BlockList, BlockParent, BlockQuery, ContentProjection, Editor};
use logic_grid_types::LogicGameOperation;
use logic_grid_types::{LogicGridContent, LogicGridDocument};
use logicgame::challenges::ChallengeId;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub(crate) struct Solution {
    pub(crate) reference: Uuid,
    pub(crate) id: Option<Uuid>,
    pub(crate) name: String,
    pub(crate) automatic: bool,
    pub(crate) completed: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Level {
    pub(crate) challenge: ChallengeId,
    pub(crate) solutions: Vec<Solution>,
    pub(crate) completed: bool,
}

#[derive(Default)]
struct Work {
    grids: HashMap<Uuid, (Scope, BlockList, ContentProjection<LogicGridContent>)>,
    hotbar: RootSetting<HotbarContent>,
}

#[derive(Clone, Copy)]
pub(crate) struct Game {
    editor: Editor,
    block: crate::logic_game::app::GameBlock,
    levels: Memo<Vec<Level>>,
    hotbar: Memo<Option<Uuid>>,
}

impl Game {
    pub(crate) fn watch(editor: Editor, block: crate::logic_game::app::GameBlock) -> Self {
        let stored = block.project(|game| {
            game.root()
                .game()
                .levels()
                .iter()
                .map(|level| (level.challenge, level.solutions.clone(), level.completed))
                .collect::<Vec<_>>()
        });
        let levels = create_signal(Vec::<Level>::new());
        let hotbar = create_signal(None::<Uuid>);
        let work = Rc::new(RefCell::new(Work::default()));
        let working = Rc::clone(&work);
        let blocks = editor.blocks();
        let host = editor.host();
        let body = owner_scope();
        create_binding(move || {
            let mut work = working.borrow_mut();
            let Work {
                grids,
                hotbar: setting,
            } = &mut *work;
            setting.find(&editor, host.client_id());
            hotbar.set(setting.block());

            let types = editor.block_types();
            let mut listed = Vec::new();
            let rows: Vec<Level> = stored
                .get()
                .into_iter()
                .map(|(challenge, solutions, completed)| Level {
                    challenge,
                    solutions: solutions
                        .into_iter()
                        .map(|reference| {
                            let id = Some(reference);
                            if let Some(id) = id {
                                listed.push(id);
                                grids.entry(id).or_insert_with(|| {
                                    let scope = body
                                        .as_ref()
                                        .and_then(ScopeContext::child)
                                        .unwrap_or_default();
                                    let (listed, grid) = scope.run(|| {
                                        (
                                            blocks.watch(BlockQuery::Block(id)),
                                            editor.content_of::<LogicGridContent>(id),
                                        )
                                    });
                                    (scope, listed, grid)
                                });
                            }
                            let handle = id.and_then(|id| grids.get(&id));
                            let label = handle.map(|(_, listed, _)| {
                                listed.read().map(|listed| {
                                    listed
                                        .into_iter()
                                        .next()
                                        .map(|info| info.label(types.as_ref()))
                                })
                            });
                            let name = match &label {
                                None => "Broken link".to_owned(),
                                Some(Ok(Some(label))) => label.name.clone(),
                                Some(Err(Pending::Failed(error))) => error.clone(),
                                Some(_) => "Loading…".to_owned(),
                            };
                            let label = label.and_then(|label| label.ok().flatten());
                            Solution {
                                reference,
                                id,
                                name,
                                automatic: label.is_none_or(|label| label.automatic),
                                completed: handle
                                    .and_then(|(_, _, grid)| {
                                        grid.read(|grid| {
                                            grid.field(ObjectId::ROOT, LogicGridDocument::COMPLETED)
                                        })
                                        .ok()
                                    })
                                    .unwrap_or(false),
                            }
                        })
                        .collect(),
                    completed,
                })
                .collect();
            grids.retain(|id, _| listed.contains(id));
            levels.set(rows);
        });
        create_effect(move || {
            let completed: Vec<ChallengeId> = levels.with(|levels| {
                levels
                    .iter()
                    .filter(|level| {
                        !level.completed
                            && level.solutions.iter().any(|solution| solution.completed)
                    })
                    .map(|level| level.challenge)
                    .collect()
            });
            for challenge in completed {
                crate::logic_game::app::operate(
                    &block,
                    LogicGameOperation::SetCompleted {
                        challenge,
                        completed: true,
                    },
                );
            }
        });
        Self {
            editor,
            block,
            levels: create_memo(move || levels.get()),
            hotbar: create_memo(move || hotbar.get()),
        }
    }

    pub(crate) fn levels(&self) -> Memo<Vec<Level>> {
        self.levels
    }

    pub(crate) fn hotbar(&self) -> Memo<Option<Uuid>> {
        self.hotbar
    }

    pub(crate) fn open_hotbar(&self) {
        if let Some(hotbar) = self.hotbar.get_untracked() {
            self.editor
                .host()
                .open_block(hotbar, HotbarContent::CONTENT_TYPE);
        }
    }

    pub(crate) fn world_hotbar(&self) -> Memo<Option<Uuid>> {
        let set = self
            .editor
            .nearest_settings(self.editor.block_id(), HotbarContent::CONTENT_TYPE);
        let world = self.editor.block_id();
        create_memo(move || {
            set.get()
                .filter(|set| set.owner == world)
                .map(|set| set.block)
        })
    }

    pub(crate) fn can_manage(&self) -> Memo<bool> {
        let (blocks, world) = (self.editor.blocks(), self.editor.block_id());
        create_memo(move || blocks.access(world).can_manage())
    }

    pub(crate) fn open_world_hotbar(&self) {
        let own = self.hotbar.get_untracked().and_then(|hotbar| {
            self.editor
                .content_of::<HotbarContent>(hotbar)
                .read(|content| content.clone())
                .ok()
        });
        let world = self.editor.block_id();
        if let Some(block) = self
            .editor
            .ensure_block_kind::<Hotbar>(world, own.unwrap_or_default())
        {
            self.editor
                .host()
                .open_block(block, HotbarContent::CONTENT_TYPE);
        }
    }

    pub(crate) fn open_solution(&self, id: Uuid) {
        self.editor
            .host()
            .open_block(id, LogicGridContent::CONTENT_TYPE);
    }

    pub(crate) fn remove(&self, challenge: ChallengeId, solution: Uuid) {
        crate::logic_game::app::operate(
            &self.block,
            LogicGameOperation::RemoveSolution {
                challenge,
                solution,
            },
        );
    }

    pub(crate) fn start(&self, challenge: ChallengeId, index: usize) {
        let id = self.editor.blocks().create_named(
            &LogicGridContent::new(&LogicGridDocument::for_challenge(challenge)),
            BlockParent::Block(self.editor.block_id()),
            format!("{} {}", challenge.name(), index + 1),
        );
        crate::logic_game::app::operate(
            &self.block,
            LogicGameOperation::InsertSolution {
                challenge,
                solution: id,
                index,
            },
        );
        self.editor
            .host()
            .open_block(id, LogicGridContent::CONTENT_TYPE);
    }
}
