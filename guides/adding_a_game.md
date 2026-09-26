# Adding a game

A deterministic game is a crate of its own that compiles to a single
WebAssembly module. The app never links a game: a module is imported into a
game module block, and the game block that references it runs the module
through the `wasmi` interpreter in `crates/tabletop_games/host`, asking it what
a player currently sees. That is the same on desktop, Android and the browser,
so a game is written and built once.

Everything about games lives under `crates/tabletop_games`, split by which
side of the WebAssembly boundary it runs on:

- `api` (`game-api`) is the contract, and is compiled into every game
  module. It holds the types the two sides exchange, the `GameHelper` a game
  is written against, the `game!` macro that declares a module's exports,
  and the build script helper each game calls. It has to build for
  wasm32-unknown-unknown, so it depends on nothing that needs a host.
- `host` (`game-host`) is the other side: it embeds `wasmi`, loads a module,
  and calls it. Only the app depends on it. It re-exports the `game-api`
  types it passes across, so a caller needs the one crate.
- `rules/<game>` is one game each - the only place a game's actual rules
  live.

## 1. Write the crate

- `crates/tabletop_games/rules/foo/Cargo.toml` — copy `tic_tac_toe`'s. The
  package name is the module's file name and the game's id, so it is written
  with underscores. It needs `crate-type = ["cdylib", "rlib"]`, `game-api`
  as a dependency and a build dependency, and `game-host` as a dev
  dependency so the tests can run the module.
- `crates/tabletop_games/rules/foo/build.rs` —
  `fn main() { game_api::build::wasm(); }`. This compiles the crate to
  wasm32-unknown-unknown and points `GAME_WASM` at the module for the tests.
- `crates/tabletop_games/rules/foo/src/lib.rs` — the game, ending in
  `game_api::game!("Foo", foo);`. The first argument is the name the module
  gives itself, which is what the game module block's editor shows; the second
  is the game function.
- `crates/tabletop_games/rules/foo/rulebook.md` — the rules in English,
  written the way a rulebook explains a game: who plays, what the pieces or
  cards are, how a turn goes, how it ends. Write this one first, because the
  code is meant to read like it.
- Root `Cargo.toml` — add `crates/tabletop_games/rules/foo` to the members.

Anything a game depends on has to build for wasm32-unknown-unknown with no
host to call into: no clock, no filesystem, no randomness. Whatever a game
would want randomness for, it seeds from the action log instead, the way
`crazy_8s` seeds its shuffle from the ids of the players who joined - two
clients replaying the same log must reach the same screen.

## 2. Write the game

A game is one straight-line function over its action log:

```rust
fn foo(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    loop {
        helper.action(
            |player| Scene::new(describe(player)).on(board(player)),
            |player, action| {
                if action(Move::new("Do the thing").click(Spot::tile(0, 0))) {
                    // the log records this move for this player
                }
            },
        )?;
    }
}
```

`helper.action` blocks on a player until the log supplies their move, so the
game reads like a normal loop rather than a replay pass. Each legal move is
offered by calling `action(Move::new(label))`; when it answers `true`, that
move is the one the log records next for that actor, so the state updates
happen right there, inline. A move is identified by its position among the
`action` calls reached for its actor, never by its label or its gesture, so
nothing a client sends can name a move it was not offered. While the game
waits in `action`, the host can also ask it for any player's screen, which
`action` answers from the same `describe` and `body` and then goes on waiting.

### What the player sees

A game never draws anything. The screen it returns is a `Scene`: a sentence
for the viewer and a `Board` (`game_api::board`) that says what is on the
table, and the Game block's editor decides what that looks like - which is
what lets a player pick their own skin for the pieces. There are two boards:

- `Grid` - columns and rows of `Tile`s, the top row first, each holding a
  stack of `Sprite` layers painted bottom first. A layer is a `Square` (a
  light or dark board square), a `Tint` over it (the last move, or a king in
  danger), a `Piece` (a `kind` such as `"x"`, `"o"`, `"disc"`, `"knight"` or
  `"man"`, and the `seat` it belongs to, which decides its colour), a
  `Card` or a `CardBack`. A grid whose every tile starts with a `Square` is
  drawn as one checkered board; any other grid as separate cells.
- `CardTable` - a list of `Pile`s, each with a label, a `PilePlace` (the deck,
  the discard pile, an extra pile, the viewer's own hand, or someone else's)
  that says where on the table it goes, a `Spread` (stacked, showing its top
  card, or fanned out) and its cards. `Table::board(viewer)` builds one for
  the usual deck, discard pile and hands, with everyone else's hand face down.

A move may carry a gesture, which is how a player makes it on the board
rather than with a button: `.click(spot)` is a click on one `Spot` (a tile, a
pile, or one card in a pile), and `.drag(from, to)` is dragging one spot onto
another - or clicking the first and then the second. The editor highlights
every spot a move can start from, and once one is picked up, every spot it
can land on. Two moves may share a gesture - playing an eight onto the
discard pile is four moves, one per suit it can call - and the editor then
asks which one was meant. A move with no gesture is a button beside the board.

The editor lays the board out in a world of its own and draws it on a
pan-and-zoom canvas, so a Game block plays the same in a tab of its own as
inside an infinite canvas. Players can mark it up the way they would a chess
board online - arrows and circles drawn with a right-drag, or with a second
finger while one is held down - and none of that reaches the game.

### The history

Every move the log records becomes a line of the game's history (`Turn`s on
the `GameScreen`), which the editor lists beside the board and steps back
through by showing the log up to that move. The line reads as the move's
label, unless `.recorded(text)` says otherwise. The history is shown to every
player, so a move whose label names something only its player may know is
recorded as what the table saw: Crazy 8s records "Drew a card", never which.
`helper.annotate(suffix)` adds to the last line once the game knows more about
it (chess marks a check `+` on the move that gave it), and
`helper.describe_last_turn(text)` replaces it. `helper.listing()` says whether
the moves being offered are listed for the viewer rather than matched against
the log, which is the only time their labels are read, so a game whose labels
are costly to write can offer unlabelled moves while it plays one out and
describe the one that was chosen.

Some of what a rulebook says in one sentence is a paragraph of Rust, so
`GameHelper` says those the short way too:

- `helper.gather(2)?` fills a table before play begins - anyone may join,
  and once two have, any of them may start - and answers with the players in
  the order they joined.
- `helper.turn(whose, yours, theirs, board, |choose| ...)` waits on the one
  player whose turn it is, who is offered the moves `choose` names; everyone
  else is only told they are waiting. `board` is what each viewer sees
  meanwhile; since `choose` changes the game's state, it looks at a copy of
  the state taken before the turn.
- `helper.game_over(|player| ...)` ends the game: every viewer is told how
  it ended and nobody is offered anything.

`game_api::cards` and `game_api::table` are the same idea for card games:
a deck of the usual 52, and a `Table` that deals it, holds each player's
hand, the draw pile and the discard pile, draws (reshuffling the discard
pile back under the face-up card when the draw pile runs out), counts
passes and passes the turn to the left. Its shuffle is seeded from the ids
of the players it deals to, so it needs no randomness of its own.

These all live in `game-api` rather than in a game, so put anything new of
the same sort there too: whenever a rulebook sentence is plain and the Rust
for it is not, the sentence belongs in the shared crate and the game keeps
only its own rules. `crazy_8s` is written that way - one `lib.rs` that
follows its `rulebook.md` section by section - so what is left in it is
what makes Crazy 8s that game: eights are wild, a played eight calls a
suit, and drawing gets you one card you may play.

The game runs once, as one long call the host pauses. `game_api::game!`
exports `play`, which runs the game function against the host: whenever
`action` needs the next move it calls the host's `next` import, and the host
pauses the whole instance there (a wasmi resumable call) until a move arrives
or a screen is asked for, then resumes it. `game-host`'s `Session` is that
paused instance: `play(action)` feeds it one move, `show(player)` asks it for a
screen, and each of those gets a fuel budget of its own, so a move that never
finishes is cut off as that move's failure however long the game has run.
wasmi cannot copy a paused instance, so there are no snapshots: looking back at
an earlier move, or opening a game again, starts a fresh session and plays the
log into it from the top, each move again on its own budget. `Game::show`
is that in one call.

### Games with pieces on a board

`crates/tabletop_games/pieces` (`game-pieces`) is the same idea for games
like chess and checkers, where the rules are mostly how each piece moves. A
piece is an implementation of `Piece`: its name (which is also its sprite
kind), its letter for notation, whether it is royal - a side may never leave
a royal piece where an enemy move could capture it - and the `Step`s it can
make from a square: where it lands, what it captures, where it stops on the
way, another piece that moves with it, what it becomes. `Rider` and `Leaper`
describe most pieces in a line; `chess` and `checkers` hold the rest. A game
is then a `Rules` - the board's size, the setup, each side's `Army` (its
name, colour, and whether it must capture when it can) and how moves are
written - and `play(helper, &RULES)` runs all of it: seating, turns, check,
the endings, resigning, and the history in the game's notation. Chess,
Checkers and Chess vs Checkers are three `Rules` over the same pieces, so a
new variant is a new setup, and a new kind of piece is one `Piece`.

## 3. Test it

Tests live in the game crate (`src/tests.rs` plus one file per test under
`src/tests/`) and drive the module rather than the Rust source:

```rust
fn show(actions: &[GameAction], player: Uuid) -> GameScreen {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| Game::load(include_bytes!(env!("GAME_WASM"))).unwrap())
        .show(actions, player)
        .unwrap()
}
```

so what is tested is the artifact that ships. Pure helpers (a deck, a win
check) are still tested directly. A guest panic has nowhere to print, so it
reaches the test as a trap rather than a message.

## 4. Getting the module into the app

Each rules crate has a `module` target, which is its wasm module:
`./scripts/buck build //crates/tabletop_games/rules/<game>:module --out <game>.wasm`
writes it. Give a new game's `BUCK` file the same three targets
`tic_tac_toe`'s has. Nothing stages them beside the app: a module reaches a
workspace as a block. Add a Game Module block, choose the `.wasm` file with the
system file picker, and the editor loads it
to check it really is a game module and names it. A Game block then references
one of those: creating one opens the block picker filtered to game modules, so
it plays a module the workspace already holds - or one imported from the picker
there and then - and the module travels with the workspace rather than with the
app.

To try a game without a second client, the Game block's "Playing as" menu
switches which player this client is: you, a guest you have already played as,
or a new guest. Each guest is a player of their own in the log, so one person
can play every side of a game. Other people are never in the menu: playing as
them would show their hand and move in their name.
