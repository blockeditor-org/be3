use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use block::Block as _;
use block_client::blocks::deterministic_game::{DeterministicGame, DeterministicGameOperation};
use block_client::blocks::game_module::GameModule;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::beui::{Context, NodeId, Rect, Vec2};
use block_editor_plugin::{BlockFilter, BlockPicker, EditorHost};
use game_host::Game;
use uuid::Uuid;

mod ui;

use block_reactive::BlockWatch;
use ui::{CreationSnapshot, GameCreationModel, GameCreationUi, GameModel, GameSnapshot, GameUi};

const INTRINSIC_SIZE: Vec2 = Vec2::new(360.0, 320.0);

struct Loaded {
    module: Uuid,
    revision: u64,
    game: Result<Arc<Game>, String>,
}

struct BlockGame {
    block: BlockHandle<DeterministicGame>,
    host: EditorHost,
}

impl GameModel for BlockGame {
    fn choose(&self, effect: Vec<u8>) {
        if self.host.editable() {
            self.block
                .operate(DeterministicGameOperation::Append { action: effect });
        }
    }
}

struct GameCreation {
    host: EditorHost,
    client: Arc<BlockClient>,
    picker: RefCell<BlockPicker>,
    chosen: Cell<Option<Uuid>>,
    error: RefCell<Option<String>>,
}

impl GameCreation {
    fn snapshot(&self) -> CreationSnapshot {
        let picked = self.picker.borrow_mut().poll(&self.host);
        match picked {
            Some(Ok(module)) => {
                self.chosen.set(Some(module.id));
                *self.error.borrow_mut() = None;
                self.host.set_creation_ready(true);
            }
            Some(Err(error)) => *self.error.borrow_mut() = Some(error),
            None => {}
        }
        let picking = self.picker.borrow().is_open();
        let chosen = self.chosen.get().map(|module| {
            self.client
                .get_block::<GameModule>(module)
                .read()
                .map_or_else(
                    || "Loading...".to_owned(),
                    |module| module.source_name().to_owned(),
                )
        });
        CreationSnapshot {
            chosen,
            picking,
            error: self.error.borrow().clone(),
        }
    }

    fn create_block(&self) -> Result<Uuid, String> {
        let module = self.chosen.get().ok_or("Choose a game module first")?;
        Ok(self
            .client
            .create_block(DeterministicGame::new(module))
            .id())
    }
}

impl GameCreationModel for GameCreation {
    fn choose_module(&self) {
        self.picker.borrow_mut().open(&self.host, module_filter());
    }
}

#[derive(Default)]
pub struct DeterministicGameApp {
    client: Option<Arc<BlockClient>>,
    game: Option<Rc<BlockGame>>,
    game_changes: Option<BlockWatch<DeterministicGame>>,
    module: Option<BlockHandle<GameModule>>,
    module_changes: Option<BlockWatch<GameModule>>,
    loaded: Option<Loaded>,
    player: Uuid,
    ui: Option<GameUi>,
    creation: Option<Rc<GameCreation>>,
    creation_ui: Option<GameCreationUi>,
    creation_snapshot: Option<CreationSnapshot>,
}

impl DeterministicGameApp {
    pub fn creation_ui(&self) -> Option<&GameCreationUi> {
        self.creation_ui.as_ref()
    }

    fn snapshot(&mut self) -> GameSnapshot {
        let Some(state) = self.game.as_ref().and_then(|game| game.block.read()) else {
            return GameSnapshot::Loading;
        };
        let module = state.module();
        let actions = state.actions().to_vec();
        drop(state);

        if self
            .module
            .as_ref()
            .is_none_or(|handle| handle.id() != module)
        {
            let client = self.client.as_ref().expect("the editor is connected");
            let handle = client.get_block::<GameModule>(module);
            let waker = self
                .game
                .as_ref()
                .expect("the editor is connected")
                .host
                .waker();
            self.module_changes = Some(BlockWatch::new(&handle, move || waker.wake()));
            self.module = Some(handle);
            self.loaded = None;
        }

        let handle = self.module.as_ref().expect("the module handle was set");
        let revision = handle.revision();
        if self
            .loaded
            .as_ref()
            .is_none_or(|loaded| loaded.module != module || loaded.revision != revision)
        {
            let Some(module_block) = handle.read() else {
                return GameSnapshot::Loading;
            };
            self.loaded = Some(Loaded {
                module,
                revision,
                game: Game::load(module_block.data()).map(Arc::new),
            });
        }

        let loaded = self.loaded.as_ref().expect("the module was just loaded");
        let game = match &loaded.game {
            Ok(game) => game,
            Err(error) => return GameSnapshot::Error(error.clone()),
        };
        match game.show(&actions, self.player) {
            Ok(screen) => GameSnapshot::screen(
                screen,
                self.game
                    .as_ref()
                    .expect("the editor is connected")
                    .host
                    .editable(),
            ),
            Err(error) => GameSnapshot::Error(error),
        }
    }
}

impl block_editor_plugin::BeuiApp for DeterministicGameApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.player = client.account_id();
        let block = client.get_block(block_id);
        self.game_changes = Some(BlockWatch::new(&block, {
            let waker = host.waker();
            move || waker.wake()
        }));
        self.game = Some(Rc::new(BlockGame { block, host }));
        self.client = Some(client);
        self.module = None;
        self.module_changes = None;
        self.loaded = None;
        self.ui = None;
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        host.set_creation_ready(false);
        let creation = Rc::new(GameCreation {
            host,
            client,
            picker: RefCell::new(BlockPicker::default()),
            chosen: Cell::new(None),
            error: RefCell::new(None),
        });
        self.creation_ui = Some(GameCreationUi::new(creation.clone()));
        self.creation = Some(creation);
        self.creation_snapshot = None;
    }

    fn creation_frame(&mut self, context: &Context, rect: Rect) {
        let Some(creation) = &self.creation else {
            return;
        };
        let snapshot = creation.snapshot();
        if self.creation_snapshot.as_ref() != Some(&snapshot) {
            if let Some(ui) = &mut self.creation_ui {
                ui.set_snapshot(snapshot.clone());
            }
            self.creation_snapshot = Some(snapshot);
        }
        if let Some(ui) = &mut self.creation_ui {
            ui.show(context, rect);
        }
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        self.creation
            .as_ref()
            .ok_or("this editor is not creating a block")?
            .create_block()
    }

    fn view(&mut self) -> NodeId {
        let snapshot = self.snapshot();
        let game = self
            .game
            .clone()
            .expect("connect is called before view is built");
        let (ui, root) = GameUi::new(game, snapshot);
        self.ui = Some(ui);
        root
    }

    fn update(&mut self) {
        let game_changed = self.game_changes.as_mut().is_some_and(BlockWatch::take);
        let module_changed = self.module_changes.as_mut().is_some_and(BlockWatch::take);
        if game_changed || module_changed {
            let snapshot = self.snapshot();
            if let Some(changes) = &mut self.module_changes {
                changes.take();
            }
            if let Some(ui) = &mut self.ui {
                ui.set_snapshot(snapshot);
            }
        }
    }

    fn intrinsic_size(&mut self) -> Option<Vec2> {
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
