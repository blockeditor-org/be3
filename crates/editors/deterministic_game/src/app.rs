use block_editor_plugin::be_block::BlockContent;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use block_editor_plugin::ContentProjection;
use block_editor_plugin::be_block::{
    DeterministicGame, DeterministicGameContent, GameModuleContent,
};
use block_editor_plugin::beui::reactive::{clone, create_signal, view};
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{BlockFilter, BlockList, BlockPicker, BlockQuery, Creation, Editor};
use game_api::GameAction;
use game_host::Game;
use uuid::Uuid;

mod ui;

use ui::{
    CreationSnapshot, Game as GameView, GameCreation as GameCreationView, GameCreationModel,
    GameModel, GameSnapshot, Seat,
};

const INTRINSIC_SIZE: Vec2 = Vec2::new(560.0, 560.0);
const GUEST: u64 = 0x6775_6573_7473;

struct Loaded {
    module: Uuid,
    revision: u64,
    game: Result<Arc<Game>, String>,
}

struct BlockGame {
    editor: Editor,
    block: Rc<ContentProjection<DeterministicGameContent>>,
    shown: Cell<Option<u64>>,
    stale: Cell<bool>,
    account: Uuid,
    player: Cell<Uuid>,
    seats: RefCell<Vec<Uuid>>,
    module: RefCell<Option<(Uuid, Rc<ContentProjection<GameModuleContent>>)>>,
    loaded: RefCell<Option<Loaded>>,
}

impl BlockGame {
    fn new(editor: Editor) -> Self {
        let account = editor.blocks().account_id();
        let block = editor.block_content::<DeterministicGameContent>();
        Self {
            editor,
            block,
            shown: Cell::new(None),
            stale: Cell::new(false),
            account,
            player: Cell::new(account),
            seats: RefCell::new(Vec::new()),
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
        self.stale.set(false);
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
        let seats = seats(self.account, &actions);
        if !seats.contains(&self.player.get()) {
            self.player.set(self.account);
        }
        let player = self.player.get();
        let seat = Seat {
            names: (0..seats.len())
                .map(|seat| seat_name(seat, seats.len()))
                .collect(),
            playing: seats
                .iter()
                .position(|seated| *seated == player)
                .expect("the player is seated"),
        };
        *self.seats.borrow_mut() = seats;
        match game.show(&actions, player) {
            Ok(screen) => {
                GameSnapshot::screen(screen, seat, self.editor.editable().get_untracked())
            }
            Err(error) => GameSnapshot::Error(error),
        }
    }

    fn played(&self) -> bool {
        self.stale.get() || self.block.revision() != self.shown.get()
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
            .operate(DeterministicGame::play(self.player.get(), effect));
    }

    fn play_as(&self, seat: usize) {
        if let Some(player) = self.seats.borrow().get(seat) {
            self.player.set(*player);
            self.stale.set(true);
        }
    }
}

struct GameCreation {
    creation: Creation,
    picker: RefCell<BlockPicker>,
    chosen: Cell<Option<Uuid>>,
    module: RefCell<Option<BlockList>>,
    error: RefCell<Option<String>>,
}

impl GameCreation {
    fn new(creation: Creation) -> Self {
        creation.set_ready(false);
        Self {
            creation,
            picker: RefCell::new(BlockPicker::default()),
            module: RefCell::new(None),
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
            let mut watched = self.module.borrow_mut();
            if watched
                .as_ref()
                .is_none_or(|list| list.query() != BlockQuery::Block(module))
            {
                *watched = Some(self.creation.blocks().watch(BlockQuery::Block(module)));
            }
            watched
                .as_ref()
                .and_then(|list| list.read().into_iter().next())
                .and_then(|info| info.name)
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
        Ok(self
            .creation
            .create(&DeterministicGameContent::new(&DeterministicGame::of(
                module,
            ))))
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

pub(crate) fn seats(account: Uuid, actions: &[GameAction]) -> Vec<Uuid> {
    let mut seats = vec![account];
    for action in actions {
        if !seats.contains(&action.actor) {
            seats.push(action.actor);
        }
    }
    let (high, low) = account.as_u64_pair();
    let newcomer = (1..)
        .map(|guest| Uuid::from_u64_pair(high ^ GUEST, low ^ guest))
        .find(|guest| !seats.contains(guest))
        .expect("there is always another guest");
    seats.push(newcomer);
    seats
}

fn seat_name(seat: usize, seats: usize) -> String {
    match seat {
        0 => "You".to_owned(),
        seat if seat + 1 == seats => "New player".to_owned(),
        seat => format!("Player {}", seat + 1),
    }
}

pub(crate) fn module_filter() -> BlockFilter {
    BlockFilter {
        name: "Game module".to_owned(),
        block_types: vec![GameModuleContent::CONTENT_TYPE.into_bytes()],
        excluded: Vec::new(),
        templates: false,
    }
}
