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
nothing a client sends can name a move it was not offered. When the log runs
out, `action` returns the screen for the viewing player, which `?` propagates
out.

### What the player sees

A game never draws anything. The screen it returns is a `Scene`: a sentence
for the viewer and a `Board` (`game_api::board`) that says what is on the
table, and the Game block's editor decides what that looks like - which is
what lets a player pick their own skin for the pieces. There are two boards:

- `Grid` - columns and rows of `Tile`s, the top row first, each holding a
  stack of `Sprite` layers painted bottom first. A layer is a `Square` (a
  light or dark board square), a `Piece` (a `kind` such as `"x"`, `"o"` or
  `"disc"`, and the `seat` it belongs to, which decides its colour), a
  `Card` or a `CardBack`. Tic-Tac-Toe and Connect Four use it.
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
asks which one was meant. A move with no gesture is a button under the board.

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

The game keeps no state between calls: the log is replayed from the top every
time, in a fresh instance, and the interpreter cuts a module off that never
returns.

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
switches which player this client is: you, anyone who has moved already, or a
new player who has not. Each is a player of their own in the log, so one person
can play every side of a game.
