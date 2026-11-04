# Adding a game

A deterministic game is a crate that compiles to one WebAssembly module.
be-host never links a game: a module is imported into a Game Module block, and
the Game block that references it runs the module through the `wasmi`
interpreter in `crates/tabletop-host`, asking what a player currently sees. A
game is written and built once for desktop, Android and the browser.

- `tabletop-api`: the contract, compiled into every game: the types both sides
  exchange, the `GameHelper` a game is written against, the `game!` macro and
  the build helper. It builds for wasm32-unknown-unknown.
- `tabletop-host`: the other side, which embeds wasmi. Only be-host depends on
  it; it re-exports the api types it passes across.
- `tabletop-pieces`: shared rules for games of pieces on a board.
- `tabletop-rules-<game>`: one game each, the only place its rules live.

## 1. Write the crate

- `crates/tabletop-rules-foo/Cargo.toml` and `BUCK`: copy tic-tac-toe's. BUCK
  builds the crate natively for tests and for wasm32 as `:module`, and points
  the tests' `GAME_WASM` at that module.
- `src/lib.rs`: the game, ending in `tabletop_api::game!("Foo", foo);` (the
  name the module gives itself, and the game function).
- `rulebook.md`: the rules in English, as a rulebook explains them. The code
  is meant to read like it, so write it first.
- Root `Cargo.toml`: add the crate to `members`.

A game has no clock, filesystem or randomness. Seed anything random from the
action log (`crazy_8s` seeds its shuffle from the joining players' ids): two
clients replaying one log must reach the same screen.

## 2. Write the game

A game is one straight-line function over its action log:

```rust
fn foo(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    loop {
        helper.action(
            |player| Scene::new(describe(player)).on(board(player)),
            |player, action| {
                if action(Move::new("Do the thing").click(Spot::tile(0, 0))) {
                    // the log records this move for this player: update the state here
                }
            },
        )?;
    }
}
```

`helper.action` blocks until the log supplies a move, so the game reads as a
normal loop rather than a replay. The game runs as one long call the host
pauses (a wasmi resumable call) at each `action` until a move arrives or a
screen is asked for; `tabletop_host::Session` is that paused instance. Since a
paused instance cannot be copied, looking back at a move or reopening a game
replays the log from the top (`Game::show`).

### What the player sees

A game draws nothing (decisions/games_describe_their_table_and_never_draw.md).
It returns a `Scene`: a sentence for the viewer and a `Board` saying what is on
the table and where; the Game block's editor decides how each thing looks, so
a player can pick their own skin. Positions are integers in units of the
game's choosing; the helpers use a 64-unit tile and a 70 by 100 card.

Moves carry gestures, `.click(spot)` or `.drag(from, to)`, and the editor
highlights where each can start and land. The editor shows no list of moves
and no status line, so a move without a gesture cannot be made (even joining
or dealing is a click on the deck); only `.control(Control::Resign)` is shown
as an icon. The editor draws the table on a pan-and-zoom canvas, so a game
plays the same in a tab or inside an infinite canvas, and players can draw
arrows and circles over it that never reach the game.

When the game ends, `game_over`'s scene with a `Scene::score` (`1-0`,
`½-½`) closes the move table and shows in a banner.

### The history

Every move the log records becomes a line of the history, laid out as a move
table beside the board; stepping back shows the log up to that move. Name the
columns once with `helper.columns(...)` (one per seat) and put each move in one
with `.column(seat)`. Write moves as tersely as the game's own notation
(`e4`, `11-15`, `K♣`).

### Shared helpers

Whenever a rulebook sentence is plain and its Rust is not, `tabletop-api` says
it, and the game keeps only its own rules: `helper.gather`, `helper.turn` and
`helper.game_over`; `Squares` for boards of tiles and `card_table` for piles
of cards; `cards` and `table` for a 52-card deck and a `Table` that deals,
draws, reshuffles and passes the turn, seeded from the players' ids.
`crazy_8s` is written this way, one `lib.rs` following its rulebook section by
section.

`tabletop-pieces` does the same for games like chess and checkers: a piece
is a `Piece` (its moves as `Step`s; `Rider` and `Leaper` cover most), and a
game is a `Rules` (board size, setup, each side's `Army`, notation) that
`play(helper, &RULES)` runs entirely, including check, endings, resigning and
history. Chess, Checkers and Chess vs Checkers are three `Rules` over the same
pieces, so a variant is a new setup and a new kind of piece is one `Piece`.

## 3. Test it

Tests live in the game crate (`src/tests/`, one per file) and drive the built
module, so what is tested is the artifact that ships:

```rust
fn show(actions: &[GameAction], player: Uuid) -> GameScreen {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| Game::load(include_bytes!(env!("GAME_WASM"))).unwrap())
        .show(actions, player)
        .unwrap()
}
```

Pure helpers (a deck, a win check) can be tested directly. A guest panic has
nowhere to print, so it reaches the test as a trap with no message.

## 4. Getting the module into the host

Every `tabletop-rules-*` crate is staged as the Games app's data
(`games/<game>.wasm`, the folder name after `tabletop-rules-` with
underscores; see `crates/be-app-deterministic-game/BUCK`), so nothing else
needs editing. Creating a Game block lists those games; choosing one copies
that exact module into the workspace as a Game Module block, so the game keeps
playing the bytes it started with whatever be-host is later built with. The
same dialog plays an existing Game Module block or a `.wasm` file:
`./scripts/buck build //crates/tabletop-rules-<game>:module --out <game>.wasm`
writes one.

To try a game alone, the Game block's "Playing as" menu switches this client
between you and guests, each a separate player in the log. Other people are
never in it: playing as them would show their hand.
