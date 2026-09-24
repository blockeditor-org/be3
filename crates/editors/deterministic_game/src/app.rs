use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use block::Block as _;
use block_client::blocks::deterministic_game::DeterministicGame as GameBlock;
use block_client::blocks::game_module::GameModule;
use block_editor_plugin::ContentProjection;
use block_editor_plugin::be_block::{
    DeterministicGame, DeterministicGameContent, GameModuleContent,
};
use block_editor_plugin::beui::reactive::{clone, create_signal, view};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{BlockFilter, BlockPicker, Creation, Editor};
use game_api::GameAction;
use game_host::Game;
use uuid::Uuid;

mod ui;

use ui::{
    CreationSnapshot, Game as GameView, GameCreation as GameCreationView, GameCreationModel,
    GameModel, GameSnapshot,
};

const INTRINSIC_SIZE: Vec2 = Vec2::new(360.0, 320.0);

struct Loaded {
    module: Uuid,
    revision: u64,
    game: Result<Arc<Game>, String>,
}

struct BlockGame {
    editor: Editor,
    block: Rc<ContentProjection<DeterministicGameContent>>,
    shown: Cell<Option<u64>>,
    player: Uuid,
    module: RefCell<Option<(Uuid, Rc<ContentProjection<GameModuleContent>>)>>,
    loaded: RefCell<Option<Loaded>>,
}

impl BlockGame {
    fn new(editor: Editor) -> Self {
        let player = editor.client().account_id();
        let block = editor.block_content::<DeterministicGameContent>();
        Self {
            editor,
            block,
            shown: Cell::new(None),
            player,
            module: RefCell::new(None),
            loaded: RefCell::new(None),
        }
    }

    fn follow(&self, module: Uuid) {
        if self
            .module
            .borrow()
            .as_ref()
            .is_some_and(|(followed, _)| *followed == module)
        {
            return;
        }
        let projection = self.editor.content_of::<GameModuleContent>(module);
        *self.module.borrow_mut() = Some((module, projection));
        *self.loaded.borrow_mut() = None;
    }

    fn snapshot(&self) -> GameSnapshot {
        self.shown.set(self.block.revision());
        let Some((module, actions)) = self.block.read(|game| {
            let game = game.root();
            let actions: Vec<GameAction> = game
                .moves
                .iter()
                .map(|played| GameAction {
                    actor: played.actor,
                    action: played.action.clone(),
                })
                .collect();
            (game.module, actions)
        }) else {
            return GameSnapshot::Loading;
        };
        let Some(module) = module else {
            return GameSnapshot::Error("this game has no module to play".to_owned());
        };

        self.follow(module);
        let (_, projection) = self
            .module
            .borrow()
            .clone()
            .expect("the module was followed");
        let Some(revision) = projection.revision() else {
            return GameSnapshot::Loading;
        };
        let stale = self
            .loaded
            .borrow()
            .as_ref()
            .is_none_or(|loaded| loaded.module != module || loaded.revision != revision);
        if stale {
            let Some(game) = projection.read(|module| Game::load(module.data()).map(Arc::new))
            else {
                return GameSnapshot::Loading;
            };
            *self.loaded.borrow_mut() = Some(Loaded {
                module,
                revision,
                game,
            });
        }

        let loaded = self.loaded.borrow();
        let loaded = loaded.as_ref().expect("the module was just loaded");
        let game = match &loaded.game {
            Ok(game) => game.clone(),
            Err(error) => return GameSnapshot::Error(error.clone()),
        };
        match game.show(&actions, self.player) {
            Ok(screen) => GameSnapshot::screen(screen, self.editor.editable().get_untracked()),
            Err(error) => GameSnapshot::Error(error),
        }
    }

    fn played(&self) -> bool {
        self.block.revision() != self.shown.get()
    }

    fn module_changed(&self) -> bool {
        let Some((_, projection)) = self.module.borrow().clone() else {
            return false;
        };
        let revision = projection.revision();
        revision.is_some()
            && self
                .loaded
                .borrow()
                .as_ref()
                .is_none_or(|loaded| Some(loaded.revision) != revision)
    }
}

impl GameModel for BlockGame {
    fn choose(&self, effect: Vec<u8>) {
        self.block
            .operate(DeterministicGame::play(self.player, effect));
    }
}

struct GameCreation {
    creation: Creation,
    picker: RefCell<BlockPicker>,
    chosen: Cell<Option<Uuid>>,
    error: RefCell<Option<String>>,
}

impl GameCreation {
    fn new(creation: Creation) -> Self {
        creation.set_ready(false);
        Self {
            creation,
            picker: RefCell::new(BlockPicker::default()),
            chosen: Cell::new(None),
            error: RefCell::new(None),
        }
    }

    fn snapshot(&self) -> CreationSnapshot {
        let picked = self.picker.borrow_mut().poll(self.creation.host());
        match picked {
            Some(Ok(module)) => {
                self.chosen.set(Some(module.id));
                *self.error.borrow_mut() = None;
                self.creation.set_ready(true);
            }
            Some(Err(error)) => *self.error.borrow_mut() = Some(error),
            None => {}
        }
        let picking = self.picker.borrow().is_open();
        let chosen = self.chosen.get().map(|module| {
            self.creation
                .client()
                .get_block::<GameModule>(module)
                .name()
                .unwrap_or_else(|| "Game module".to_owned())
        });
        CreationSnapshot {
            chosen,
            picking,
            error: self.error.borrow().clone(),
        }
    }

    fn create_block(&self) -> Result<Uuid, String> {
        let module = self.chosen.get().ok_or("Choose a game module first")?;
        let block = self
            .creation
            .client()
            .create_block(GameBlock::with_references(vec![module]));
        self.creation.seed_content(
            block.id(),
            &DeterministicGameContent::new(&DeterministicGame::of(module)),
        );
        Ok(block.id())
    }
}

impl GameCreationModel for GameCreation {
    fn choose_module(&self) {
        self.picker
            .borrow_mut()
            .open(self.creation.host(), module_filter());
    }
}

pub struct DeterministicGameApp;

impl block_editor_plugin::BeuiApp for DeterministicGameApp {
    fn view(editor: Editor) -> NodeId {
        let game = Rc::new(BlockGame::new(editor.clone()));
        let (snapshot, set_snapshot) = create_signal(game.snapshot());
        editor.each_frame(clone!(game -> move || {
            if game.played() || game.module_changed() {
                set_snapshot.set(game.snapshot());
            }
        }));
        let model: Rc<dyn GameModel> = game;
        view! {
            <GameView game={model} snapshot={snapshot} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        let dialog = Rc::new(GameCreation::new(creation.clone()));
        let (snapshot, set_snapshot) = create_signal(CreationSnapshot::default());
        creation.each_frame(clone!(dialog -> move || set_snapshot.set(dialog.snapshot())));
        creation.on_create(clone!(dialog -> move || dialog.create_block()));
        let model: Rc<dyn GameCreationModel> = dialog;
        view! {
            <GameCreationView creation={model} snapshot={snapshot} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

pub(crate) fn module_filter() -> BlockFilter {
    BlockFilter {
        name: "Game module".to_owned(),
        block_types: vec![GameModule::TYPE_ID.into_bytes()],
        excluded: Vec::new(),
        templates: false,
    }
}
