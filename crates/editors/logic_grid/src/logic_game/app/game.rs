use block_editor_beui::be_block::{BlockContent, HotbarContent};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::be_block::logic_game::LogicGameOperation;
use block_editor_beui::be_block::{LogicGridContent, LogicGridDocument, ObjectId};
use block_editor_beui::beui::reactive::{Memo, create_effect, create_memo, create_signal};
use block_editor_beui::root_settings::RootSetting;
use block_editor_beui::{BlockList, BlockParent, BlockQuery, ContentProjection, Editor};
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
    grids: HashMap<Uuid, (BlockList, Rc<ContentProjection<LogicGridContent>>)>,
    hotbar: RootSetting<HotbarContent>,
}

pub(crate) struct Game {
    editor: Editor,
    block: crate::logic_game::app::GameBlock,
    levels: Memo<Vec<Level>>,
    hotbar: Memo<Option<Uuid>>,
}

impl Game {
    pub(crate) fn watch(editor: &Editor, block: crate::logic_game::app::GameBlock) -> Self {
        let stored = block.project(|game| {
            game.root()
                .game()
                .levels()
                .iter()
                .map(|level| (level.challenge, level.solutions.clone(), level.completed))
                .collect::<Vec<_>>()
        });
        let (levels, set_levels) = create_signal(Vec::<Level>::new());
        let (hotbar, set_hotbar) = create_signal(None::<Uuid>);
        let work = Rc::new(RefCell::new(Work::default()));
        let working = Rc::clone(&work);
        let blocks = editor.blocks();
        let host = editor.host().clone();
        let game = Rc::clone(&block);
        let reader = editor.clone();
        create_effect(move || {
            let mut work = working.borrow_mut();
            let Work { grids, hotbar } = &mut *work;
            hotbar.find(&reader, host.client_id());
            set_hotbar.set(hotbar.block());

            let types = reader.block_types();
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
                                    (
                                        blocks.watch(BlockQuery::Block(id)),
                                        reader.content_of::<LogicGridContent>(id),
                                    )
                                });
                            }
                            let handle = id.and_then(|id| grids.get(&id));
                            let label = handle.and_then(|(listed, _)| {
                                listed
                                    .read()
                                    .into_iter()
                                    .next()
                                    .map(|info| info.label(types.as_ref()))
                            });
                            Solution {
                                reference,
                                id,
                                name: label.as_ref().map_or_else(
                                    || match id.is_some() {
                                        true => "Loading…".to_owned(),
                                        false => "Broken link".to_owned(),
                                    },
                                    |label| label.name.clone(),
                                ),
                                automatic: label.is_none_or(|label| label.automatic),
                                completed: handle
                                    .and_then(|(_, grid)| {
                                        grid.read(|grid| {
                                            grid.field(ObjectId::ROOT, LogicGridDocument::COMPLETED)
                                        })
                                    })
                                    .unwrap_or(false),
                            }
                        })
                        .collect(),
                    completed,
                })
                .collect();
            grids.retain(|id, _| listed.contains(id));
            for level in &rows {
                if level.solutions.iter().any(|solution| solution.completed) && !level.completed {
                    crate::logic_game::app::operate(
                        &game,
                        LogicGameOperation::SetCompleted {
                            challenge: level.challenge,
                            completed: true,
                        },
                    );
                }
            }
            set_levels.set(rows);
        });
        Self {
            editor: editor.clone(),
            block,
            levels: create_memo(move || levels.get()),
            hotbar: create_memo(move || hotbar.get()),
        }
    }

    pub(crate) fn levels(&self) -> Memo<Vec<Level>> {
        self.levels.clone()
    }

    pub(crate) fn hotbar(&self) -> Memo<Option<Uuid>> {
        self.hotbar.clone()
    }

    pub(crate) fn open_hotbar(&self) {
        if let Some(hotbar) = self.hotbar.get_untracked() {
            self.editor
                .host()
                .open_block(hotbar, HotbarContent::CONTENT_TYPE);
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
