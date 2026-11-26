use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::beui::styled::Root;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_sdk_beui::ContentProjection;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{
    BlockFilter, BlockList, BlockParent, BlockQuery, Creation, Editor, PickAnchor, answer,
};
use be_type_deterministic_game::{
    DeterministicGame, DeterministicGameContent, GameModuleContent, GameModuleFile,
};
use tabletop_api::GameAction;
use tabletop_host::{Game, Session};
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
            let game = match projection.payload().get() {
                Ok(data) => Game::load(&data),
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

/// A staged game read and loaded, named as the game names itself.
#[derive(PartialEq)]
struct StagedGame {
    name: String,
    data: Rc<Vec<u8>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Chosen {
    Staged(usize),
    Module(Uuid),
}

struct GameCreation {
    this: Cell<Option<Stored<GameCreation>>>,
    creation: Creation,
    /// The games the app was staged with, as the files under `games/` that hold them.
    files: AsyncMemo<Vec<String>>,
    /// Each staged game's file read and loaded, asked for once the listing names it.
    games: KeyedItems<String, AsyncMemo<StagedGame>>,
    chosen: Signal<Option<Chosen>>,
    module: RefCell<Option<BlockList>>,
}

impl GameCreation {
    fn new(creation: Creation) -> Self {
        let host = creation.host().clone();
        let files = create_async_memo(move || {
            let asked = host.list_data();
            async move {
                Ok(answer(asked)
                    .await?
                    .into_iter()
                    .filter(|file| file.starts_with(STAGED_GAMES) && file.ends_with(".wasm"))
                    .collect())
            }
        });
        let host = creation.host().clone();
        let games = KeyedItems::new(move |file: String| {
            let host = host.clone();
            create_async_memo(move || {
                let asked = host.read_data(file.clone());
                async move {
                    let data = answer(asked).await?;
                    Game::load(&data)
                        .map(|game| StagedGame {
                            name: game.name().to_owned(),
                            data: Rc::new(data),
                        })
                        .map_err(Pending::Failed)
                }
            })
        });
        Self {
            this: Cell::new(None),
            files,
            games,
            module: RefCell::new(None),
            chosen: create_signal(None),
            creation,
        }
    }

    fn snapshot(&self) -> CreationSnapshot {
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
        let files = self.files.get();
        let games = files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, file)| {
                self.games.entry(file.clone()).with(|game| match game {
                    Ok(game) => StagedChoice {
                        index,
                        name: game.name.clone(),
                        state: StagedState::Ready {
                            chosen: chosen == Some(Chosen::Staged(index)),
                        },
                    },
                    Err(pending) => StagedChoice {
                        index,
                        name: file_name(file).to_owned(),
                        state: match pending {
                            Pending::Loading => StagedState::Loading,
                            Pending::Failed(error) => StagedState::Failed(error.clone()),
                        },
                    },
                })
            })
            .collect();
        CreationSnapshot {
            games,
            listing: files == Err(Pending::Loading),
            module,
            error: match files {
                Err(Pending::Failed(error)) => Some(error),
                _ => None,
            },
            ready: chosen.is_some(),
        }
    }

    fn create_block(&self) -> Result<Uuid, String> {
        match self.chosen.get_untracked().ok_or("Choose a game first")? {
            Chosen::Module(module) => {
                Ok(self
                    .creation
                    .create(&DeterministicGameContent::new(&DeterministicGame::of(
                        module,
                    ))))
            }
            Chosen::Staged(index) => {
                let file = self
                    .files
                    .with_untracked(|files| files.as_ref().ok()?.get(index).cloned())
                    .ok_or("That game is no longer offered")?;
                let data = self
                    .games
                    .get(&file)
                    .and_then(|game| {
                        game.with_untracked(|game| {
                            game.as_ref().ok().map(|game| Rc::clone(&game.data))
                        })
                    })
                    .ok_or_else(|| format!("{} has not loaded", file_name(&file)))?;
                let blocks = self.creation.blocks();
                let module = blocks.create(
                    &GameModuleFile::from_file(file_name(&file), data.to_vec()),
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
        self.chosen.set(Some(Chosen::Staged(index)));
    }

    fn choose_module(&self) {
        let this = self.this.get();
        self.creation
            .pick_block(module_filter(), PickAnchor::Centre, move |picked| {
                let Some(creation) = this.and_then(|this| this.try_value()) else {
                    return;
                };
                creation.chosen.set(Some(Chosen::Module(picked.id())));
            });
    }

    fn accept(&self) {
        self.creation.accept();
    }

    fn cancel(&self) {
        self.creation.cancel();
    }
}

pub struct DeterministicGameRoot;

#[component]
impl be_sdk_beui::BeuiRoot for DeterministicGameRoot {
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
        dialog.value().this.set(Some(dialog));
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
    }
}
