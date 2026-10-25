use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::styled::Root;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use block_editor_beui::ContentProjection;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{BlockFilter, BlockList, BlockParent, BlockQuery, Creation, Editor};
use deterministic_game_types::{
    DeterministicGame, DeterministicGameContent, GameModuleContent, GameModuleFile,
};
use game_api::GameAction;
use game_host::{Game, Session};
use uuid::Uuid;

pub(crate) mod ui;

use ui::{
    CreationSnapshot, Ending, Game as GameView, GameCreation as GameCreationView,
    GameCreationModel, GameModel, GameSnapshot, Seat, StagedChoice, StagedState, Table, Turn,
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
    block: ContentProjection<DeterministicGameContent>,
    account: Uuid,
    player: Signal<Uuid>,
    shown: Signal<Option<usize>>,
    seats: RefCell<Vec<Uuid>>,
    module: RefCell<Option<(Uuid, ContentProjection<GameModuleContent>)>>,
    loaded: RefCell<Option<Loaded>>,
}

impl BlockGame {
    fn new(editor: Editor) -> Self {
        let account = editor.blocks().account_id();
        let block = editor.block_content::<DeterministicGameContent>();
        let player = create_signal(account);
        let shown = create_signal(None);
        Self {
            editor,
            block,
            account,
            player,
            shown,
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
        let read = self.block.read(|game| {
            let game = game.root();
            let actions: Vec<GameAction> = game
                .moves
                .iter()
                .map(|played| GameAction {
                    actor: played.actor,
                    action: played.action.to_vec(),
                })
                .collect();
            (game.module, actions)
        });
        let (module, actions) = match read {
            Ok(read) => read,
            Err(pending) => return waiting(pending),
        };
        let Some(module) = module else {
            return GameSnapshot::Error("this game has no module to play".to_owned());
        };

        self.follow(module);
        let (_, projection) = self.module.borrow().expect("the module was followed");
        let revision = match projection.revision() {
            Ok(revision) => revision,
            Err(pending) => return waiting(pending),
        };
        let stale = self
            .loaded
            .borrow()
            .as_ref()
            .is_none_or(|loaded| loaded.module != module || loaded.revision != revision);
        if stale {
            let game = match projection.read(|module| Game::load(module.data())) {
                Ok(game) => game,
                Err(pending) => return waiting(pending),
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
            self.player.set(player);
        }
    }

    fn show_turns(&self, turns: Option<usize>) {
        self.shown.set(turns);
    }
}

const STAGED_GAMES: &str = "games/";

enum Staged {
    Reading,
    Loaded(Rc<Vec<u8>>),
    Failed(String),
}

struct StagedGame {
    file: String,
    name: String,
    staged: Staged,
}

#[derive(Clone, Copy, PartialEq)]
enum Chosen {
    Staged(usize),
    Module(Uuid),
}

struct GameCreation {
    this: Cell<Option<Stored<GameCreation>>>,
    creation: Creation,
    picking: Cell<bool>,
    listing: Cell<bool>,
    staged: RefCell<Vec<StagedGame>>,
    listing_error: RefCell<Option<String>>,
    chosen: Cell<Option<Chosen>>,
    module: RefCell<Option<BlockList>>,
    error: RefCell<Option<String>>,
    changed: Signal<u64>,
}

impl GameCreation {
    fn new(creation: Creation) -> Self {
        creation.set_ready(false);
        Self {
            this: Cell::new(None),
            changed: create_signal(0),
            picking: Cell::new(false),
            listing: Cell::new(true),
            staged: RefCell::new(Vec::new()),
            listing_error: RefCell::new(None),
            module: RefCell::new(None),
            chosen: Cell::new(None),
            error: RefCell::new(None),
            creation,
        }
    }

    fn start(this: Stored<GameCreation>) {
        let creation = this.value();
        creation.this.set(Some(this));
        let host = creation.creation.host().clone();
        creation.creation.then(host.list_data(), move |listing| {
            if let Some(creation) = this.try_value() {
                creation.listed(listing);
            }
        });
    }

    fn changed(&self) {
        self.changed.update(|changed| *changed += 1);
    }

    fn listed(&self, listing: Result<Vec<String>, String>) {
        self.listing.set(false);
        match listing {
            Ok(files) => {
                let files: Vec<String> = files
                    .into_iter()
                    .filter(|file| file.starts_with(STAGED_GAMES) && file.ends_with(".wasm"))
                    .collect();
                *self.staged.borrow_mut() = files
                    .iter()
                    .map(|file| StagedGame {
                        name: file_name(file).to_owned(),
                        staged: Staged::Reading,
                        file: file.clone(),
                    })
                    .collect();
                let this = self.this.get();
                let host = self.creation.host().clone();
                for (index, file) in files.into_iter().enumerate() {
                    self.creation.then(host.read_data(file), move |read| {
                        if let Some(creation) = this.and_then(|this| this.try_value()) {
                            creation.read(index, read);
                        }
                    });
                }
            }
            Err(error) => *self.listing_error.borrow_mut() = Some(error),
        }
        self.changed();
    }

    fn read(&self, index: usize, read: Result<Vec<u8>, String>) {
        if let Some(game) = self.staged.borrow_mut().get_mut(index) {
            game.staged = match read {
                Ok(data) => match Game::load(&data) {
                    Ok(loaded) => {
                        loaded.name().clone_into(&mut game.name);
                        Staged::Loaded(Rc::new(data))
                    }
                    Err(error) => Staged::Failed(error),
                },
                Err(error) => Staged::Failed(error),
            };
        }
        self.changed();
    }

    fn snapshot(&self) -> CreationSnapshot {
        self.changed.get();
        let picking = self.picking.get();
        let chosen = self.chosen.get();
        let module = match chosen {
            Some(Chosen::Module(module)) => {
                let mut watched = self.module.borrow_mut();
                if watched
                    .as_ref()
                    .is_none_or(|list| list.query() != BlockQuery::Block(module))
                {
                    *watched = Some(self.creation.blocks().watch(BlockQuery::Block(module)));
                }
                Some(
                    watched
                        .as_ref()
                        .and_then(|list| list.read().ok()?.into_iter().next())
                        .and_then(|info| info.name())
                        .unwrap_or_else(|| "Game module".to_owned()),
                )
            }
            _ => None,
        };
        let games = self
            .staged
            .borrow()
            .iter()
            .enumerate()
            .map(|(index, game)| StagedChoice {
                index,
                name: game.name.clone(),
                state: match &game.staged {
                    Staged::Reading => StagedState::Loading,
                    Staged::Loaded(_) => StagedState::Ready {
                        chosen: chosen == Some(Chosen::Staged(index)),
                    },
                    Staged::Failed(error) => StagedState::Failed(error.clone()),
                },
            })
            .collect();
        CreationSnapshot {
            games,
            listing: self.listing.get(),
            module,
            picking,
            error: self
                .error
                .borrow()
                .clone()
                .or_else(|| self.listing_error.borrow().clone()),
        }
    }

    fn choose(&self, chosen: Chosen) {
        self.chosen.set(Some(chosen));
        *self.error.borrow_mut() = None;
        self.creation.set_ready(true);
    }

    fn create_block(&self) -> Result<Uuid, String> {
        match self.chosen.get().ok_or("Choose a game first")? {
            Chosen::Module(module) => {
                Ok(self
                    .creation
                    .create(&DeterministicGameContent::new(&DeterministicGame::of(
                        module,
                    ))))
            }
            Chosen::Staged(index) => {
                let staged = self.staged.borrow();
                let game = staged.get(index).ok_or("That game is no longer offered")?;
                let Staged::Loaded(data) = &game.staged else {
                    return Err(format!("{} has not loaded", game.name));
                };
                let blocks = self.creation.blocks();
                let module = blocks.create(
                    &GameModuleFile::from_file(file_name(&game.file), data.to_vec()),
                    BlockParent::Detached,
                );
                let block =
                    self.creation
                        .create(&DeterministicGameContent::new(&DeterministicGame::of(
                            module,
                        )));
                blocks.set_parent(module, BlockParent::Block(block));
                Ok(block)
            }
        }
    }
}

fn waiting(pending: Pending) -> GameSnapshot {
    match pending {
        Pending::Loading => GameSnapshot::Loading,
        Pending::Failed(error) => GameSnapshot::Error(error),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

impl GameCreationModel for GameCreation {
    fn choose_staged(&self, index: usize) {
        self.choose(Chosen::Staged(index));
        self.changed();
    }

    fn choose_module(&self) {
        let this = self.this.get();
        let host = self.creation.host().clone();
        self.picking.set(true);
        self.creation
            .then(host.pick_block(module_filter()), move |picked| {
                let Some(creation) = this.and_then(|this| this.try_value()) else {
                    return;
                };
                creation.picking.set(false);
                match picked {
                    Ok(Some(module)) => creation.choose(Chosen::Module(module.id)),
                    Ok(None) => {}
                    Err(error) => *creation.error.borrow_mut() = Some(error),
                }
                creation.changed();
            });
        self.changed();
    }
}

pub struct DeterministicGameApp;

#[component]
impl block_editor_beui::BeuiApp for DeterministicGameApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let game = store(BlockGame::new(editor));
        let snapshot = create_signal(GameSnapshot::Loading);
        create_effect(move || snapshot.set(game.value().snapshot()));
        let model: Rc<dyn GameModel> = game.value();
        let model = store(model);
        view! {
            <Root>
                <GameView editor game={model} snapshot={snapshot.read_only()} />
            </Root>
        }
    }

    #[component]
    fn Creation(creation: Creation) -> NodeId {
        let dialog = store(GameCreation::new(creation.clone()));
        GameCreation::start(dialog);
        let snapshot = create_signal(CreationSnapshot::default());
        create_effect(move || snapshot.set(dialog.value().snapshot()));
        let maker = dialog.value();
        creation.on_create(move || maker.create_block());
        let model: Rc<dyn GameCreationModel> = dialog.value();
        let model = store(model);
        view! {
            <GameCreationView creation={model} snapshot={snapshot.read_only()} />
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
        place: None,
        anchor: None,
    }
}
