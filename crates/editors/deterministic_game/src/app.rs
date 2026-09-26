use block_editor_beui::be_block::BlockContent;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use block_editor_beui::ContentProjection;
use block_editor_beui::be_block::{DeterministicGame, DeterministicGameContent, GameModuleContent};
use block_editor_beui::beui::reactive::{
    ReadSignal, WriteSignal, clone, create_effect, create_signal, view,
};
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockFilter, BlockList, BlockPicker, BlockQuery, Creation, Editor};
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
    account: Uuid,
    player: ReadSignal<Uuid>,
    set_player: WriteSignal<Uuid>,
    seats: RefCell<Vec<Uuid>>,
    module: RefCell<Option<(Uuid, Rc<ContentProjection<GameModuleContent>>)>>,
    loaded: RefCell<Option<Loaded>>,
}

impl BlockGame {
    fn new(editor: Editor) -> Self {
        let account = editor.blocks().account_id();
        let block = editor.block_content::<DeterministicGameContent>();
        let (player, set_player) = create_signal(account);
        Self {
            editor,
            block,
            account,
            player,
            set_player,
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
        let player = Some(self.player.get())
            .filter(|player| seats.contains(player))
            .unwrap_or(self.account);
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
            Ok(screen) => GameSnapshot::screen(screen, seat, self.editor.editable().get()),
            Err(error) => GameSnapshot::Error(error),
        }
    }
}

impl GameModel for BlockGame {
    fn choose(&self, effect: Vec<u8>) {
        let player = self.player.get_untracked();
        let player = if self.seats.borrow().contains(&player) {
            player
        } else {
            self.account
        };
        self.block.operate(DeterministicGame::play(player, effect));
    }

    fn play_as(&self, seat: usize) {
        let player = self.seats.borrow().get(seat).copied();
        if let Some(player) = player {
            self.set_player.set(player);
        }
    }
}

struct GameCreation {
    creation: Creation,
    picker: RefCell<BlockPicker>,
    chosen: Cell<Option<Uuid>>,
    module: RefCell<Option<BlockList>>,
    error: RefCell<Option<String>>,
    opened: ReadSignal<u64>,
    set_opened: WriteSignal<u64>,
}

impl GameCreation {
    fn new(creation: Creation) -> Self {
        creation.set_ready(false);
        let (opened, set_opened) = create_signal(0);
        Self {
            opened,
            set_opened,
            creation,
            picker: RefCell::new(BlockPicker::default()),
            module: RefCell::new(None),
            chosen: Cell::new(None),
            error: RefCell::new(None),
        }
    }

    fn snapshot(&self) -> CreationSnapshot {
        self.opened.get();
        self.creation.replies().get();
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
        self.set_opened.update(|opened| *opened += 1);
    }
}

pub struct DeterministicGameApp;

impl block_editor_beui::BeuiApp for DeterministicGameApp {
    fn view(editor: Editor) -> NodeId {
        let game = Rc::new(BlockGame::new(editor.clone()));
        let (snapshot, set_snapshot) = create_signal(GameSnapshot::Loading);
        create_effect(clone!(game -> move || set_snapshot.set(game.snapshot())));
        let model: Rc<dyn GameModel> = game;
        view! {
            <GameView game={model} snapshot={snapshot} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        let dialog = Rc::new(GameCreation::new(creation.clone()));
        let (snapshot, set_snapshot) = create_signal(CreationSnapshot::default());
        create_effect(clone!(dialog -> move || set_snapshot.set(dialog.snapshot())));
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
