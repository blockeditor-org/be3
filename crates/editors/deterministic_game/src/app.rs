use block_editor_beui::be_block::BlockContent;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use block_editor_beui::ContentProjection;
use block_editor_beui::be_block::{DeterministicGame, DeterministicGameContent, GameModuleContent};
use block_editor_beui::beui::reactive::{
    ReadSignal, WriteSignal, clone, create_effect, create_signal, view,
};
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockFilter, BlockList, BlockPicker, BlockQuery, Creation, Editor};
use game_api::GameAction;
use game_host::{Game, Session};
use uuid::Uuid;

pub(crate) mod ui;

use ui::{
    CreationSnapshot, Ending, Game as GameView, GameCreation as GameCreationView,
    GameCreationModel, GameModel, GameSnapshot, Seat, Table, Turn,
};

const INTRINSIC_SIZE: Vec2 = Vec2::new(560.0, 560.0);
const GUEST: u64 = 0x6775_6573_7473;

struct Loaded {
    module: Uuid,
    revision: u64,
    game: Result<Game, String>,
    live: Option<Session>,
    past: Option<Session>,
}

struct BlockGame {
    editor: Editor,
    block: Rc<ContentProjection<DeterministicGameContent>>,
    account: Uuid,
    player: ReadSignal<Uuid>,
    set_player: WriteSignal<Uuid>,
    shown: ReadSignal<Option<usize>>,
    set_shown: WriteSignal<Option<usize>>,
    seats: RefCell<Vec<Uuid>>,
    module: RefCell<Option<(Uuid, Rc<ContentProjection<GameModuleContent>>)>>,
    loaded: RefCell<Option<Loaded>>,
}

impl BlockGame {
    fn new(editor: Editor) -> Self {
        let account = editor.blocks().account_id();
        let block = editor.block_content::<DeterministicGameContent>();
        let (player, set_player) = create_signal(account);
        let (shown, set_shown) = create_signal(None);
        Self {
            editor,
            block,
            account,
            player,
            set_player,
            shown,
            set_shown,
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
            let Some(game) = projection.read(|module| Game::load(module.data())) else {
                return GameSnapshot::Loading;
            };
            *self.loaded.borrow_mut() = Some(Loaded {
                module,
                revision,
                game,
                live: None,
                past: None,
            });
        }

        let mut loaded = self.loaded.borrow_mut();
        let Loaded {
            game, live, past, ..
        } = loaded.as_mut().expect("the module was just loaded");
        let game = match game {
            Ok(game) => &*game,
            Err(error) => return GameSnapshot::Error(error.clone()),
        };
        let seats = seats(self.account, &actions);
        let player = Some(self.player.get())
            .filter(|player| seats.contains(player))
            .unwrap_or(self.account);
        let newcomer = seats.last().copied();
        let seat = Seat {
            names: seats
                .iter()
                .map(|seated| match Some(*seated) == newcomer {
                    true => "New player".to_owned(),
                    false => self.name(*seated, &actions),
                })
                .collect(),
            playing: seats
                .iter()
                .position(|seated| *seated == player)
                .expect("the player is seated"),
        };
        *self.seats.borrow_mut() = seats;
        let live = match follow(game, live, &actions).and_then(|session| session.show(player)) {
            Ok(screen) => screen,
            Err(error) => return GameSnapshot::Error(error),
        };
        let history: Vec<Turn> = live
            .history
            .iter()
            .map(|turn| Turn {
                description: turn.description.clone(),
                player: self.name(turn.actor, &actions),
                column: turn.column,
            })
            .collect();
        let table = Table {
            columns: live.columns.to_vec(),
            history,
            ending: live.ending.as_ref().map(|ending| Ending {
                score: ending.score.clone(),
                description: live.description.clone(),
            }),
        };
        let shown = self
            .shown
            .get()
            .filter(|shown| *shown < table.history.len());
        let editable = self.editor.editable().get();
        let Some(shown) = shown else {
            return GameSnapshot::screen(live, seat, editable, table, None);
        };
        let until = match shown {
            0 => 0,
            shown => live.history[shown - 1].entry as usize + 1,
        };
        match follow(game, past, &actions[..until]).and_then(|session| session.show(player)) {
            Ok(mut past) => {
                past.actions.clear();
                GameSnapshot::screen(past, seat, false, table, Some(shown))
            }
            Err(error) => GameSnapshot::Error(error),
        }
    }

    fn name(&self, actor: Uuid, actions: &[GameAction]) -> String {
        if actor == self.account {
            return "You".to_owned();
        }
        if let Some(guest) = guest_number(self.account, actor) {
            return format!("Guest {guest}");
        }
        let mut others: Vec<Uuid> = Vec::new();
        for action in actions {
            if action.actor != self.account
                && guest_number(self.account, action.actor).is_none()
                && !others.contains(&action.actor)
            {
                others.push(action.actor);
            }
        }
        let position = others
            .iter()
            .position(|other| *other == actor)
            .unwrap_or(others.len());
        format!("Player {}", position + 2)
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

    fn show_turns(&self, turns: Option<usize>) {
        self.set_shown.set(turns);
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
            <GameView editor game={model} snapshot={snapshot} />
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

fn follow<'a>(
    game: &Game,
    slot: &'a mut Option<Session>,
    actions: &[GameAction],
) -> Result<&'a mut Session, String> {
    if slot
        .as_ref()
        .is_none_or(|session| !actions.starts_with(session.played()))
    {
        *slot = Some(game.start()?);
    }
    let session = slot.as_mut().expect("a session was just started");
    let played = session.played().len();
    for action in &actions[played..] {
        session.play(action)?;
    }
    Ok(session)
}

pub(crate) fn seats(account: Uuid, actions: &[GameAction]) -> Vec<Uuid> {
    let mut guests: Vec<u64> = actions
        .iter()
        .filter_map(|action| guest_number(account, action.actor))
        .collect();
    guests.sort_unstable();
    guests.dedup();
    let newcomer = (1..)
        .find(|guest| !guests.contains(guest))
        .expect("there is always another guest");
    let mut seats = vec![account];
    seats.extend(guests.into_iter().map(|guest| guest_of(account, guest)));
    seats.push(guest_of(account, newcomer));
    seats
}

fn guest_of(account: Uuid, guest: u64) -> Uuid {
    let (high, low) = account.as_u64_pair();
    Uuid::from_u64_pair(high ^ GUEST, low ^ guest)
}

fn guest_number(account: Uuid, actor: Uuid) -> Option<u64> {
    let (high, low) = account.as_u64_pair();
    let (actor_high, actor_low) = actor.as_u64_pair();
    (actor_high == high ^ GUEST && actor_low != low).then_some(actor_low ^ low)
}

pub(crate) fn module_filter() -> BlockFilter {
    BlockFilter {
        name: "Game module".to_owned(),
        block_types: vec![GameModuleContent::CONTENT_TYPE.into_bytes()],
        excluded: Vec::new(),
        templates: false,
    }
}
