use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use block::{Block, BlockParent};
use block_client::BlockHandle;
use block_client::blocks::hotbar::Hotbar;
use block_client::blocks::logic_grid::LogicGrid;
use block_client::root_settings::RootSetting;
use block_editor_plugin::ContentProjection;
use block_editor_plugin::Editor;
use block_editor_plugin::be_block::logic_game::LogicGameOperation;
use block_editor_plugin::be_block::{LogicGridContent, LogicGridDocument, ObjectId};
use block_editor_plugin::beui::reactive::{Memo, create_memo, create_signal};
use block_editor_plugin::block_ui::BlockLabel;
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
    grids: HashMap<
        Uuid,
        (
            BlockHandle<LogicGrid>,
            Rc<ContentProjection<LogicGridContent>>,
        ),
    >,
    hotbar: Option<RootSetting<Hotbar>>,
}

pub(crate) struct Game {
    editor: Editor,
    block: crate::app::GameBlock,
    levels: Memo<Vec<Level>>,
    hotbar: Memo<Option<Uuid>>,
}

impl Game {
    pub(crate) fn watch(editor: &Editor, block: crate::app::GameBlock) -> Self {
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
        let each_frame = Rc::clone(&work);
        let client = Arc::clone(editor.client());
        let host = editor.host().clone();
        let game = Rc::clone(&block);
        let reader = editor.clone();
        editor.each_frame(move || {
            let mut work = each_frame.borrow_mut();
            let Work { grids, hotbar } = &mut *work;
            let setting = hotbar.get_or_insert_with(|| RootSetting::new(&client));
            setting.find(&client, host.client_id());
            set_hotbar.set(setting.block().map(BlockHandle::id));

            let types = host.block_types();
            let mut listed = Vec::new();
            let rows: Vec<Level> = stored
                .get_untracked()
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
                                        client.get_block::<LogicGrid>(id),
                                        reader.content_of::<LogicGridContent>(id),
                                    )
                                });
                            }
                            let handle = id.and_then(|id| grids.get(&id));
                            let label = handle
                                .map(|(handle, _)| BlockLabel::for_handle(types.as_ref(), handle));
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
                    crate::app::operate(
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
            self.editor.host().open_block(hotbar, Hotbar::TYPE_ID);
        }
    }

    pub(crate) fn open_solution(&self, id: Uuid) {
        self.editor.host().open_block(id, LogicGrid::TYPE_ID);
    }

    pub(crate) fn remove(&self, challenge: ChallengeId, solution: Uuid) {
        crate::app::operate(
            &self.block,
            LogicGameOperation::RemoveSolution {
                challenge,
                solution,
            },
        );
    }

    pub(crate) fn start(&self, challenge: ChallengeId, index: usize) {
        let solution = self
            .editor
            .create_with_content::<LogicGrid, _>(&LogicGridContent::new(
                &LogicGridDocument::for_challenge(challenge),
            ));
        solution.set_name(format!("{} {}", challenge.name(), index + 1));
        solution.set_parent(BlockParent::Uuid(self.editor.block_id()));
        let id = solution.id();
        crate::app::operate(
            &self.block,
            LogicGameOperation::InsertSolution {
                challenge,
                solution: id,
                index,
            },
        );
        self.editor.host().open_block(id, LogicGrid::TYPE_ID);
    }
}
