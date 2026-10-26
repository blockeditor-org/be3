use std::rc::Rc;

use block_editor_beui::Fit;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_CHECK, ICON_FLAG, ICON_X};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{self, ChoiceOption};
use block_editor_beui::{Editor, LoadingPane};
use game_api::{Board, Control, GameActionOption, GameScreen, Gesture};

mod annotations;
mod board;
mod hand;
mod layout;
mod motion;
pub(crate) mod moves;
mod play;
mod skin;

use board::Stage;
use layout::Layout;
use moves::MoveTable;
use play::Play;

const SIDEBAR_WIDTH: f32 = 280.0;
const PANEL_PADDING: f32 = 14.0;
const SECTION_SPACING: f32 = 12.0;
const BANNER_MARGIN: f32 = 12.0;

pub(crate) trait GameModel {
    fn choose(&self, effect: Vec<u8>);
    fn play_as(&self, seat: usize);
    fn show_turns(&self, turns: Option<usize>);
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) struct Action {
    index: usize,
    label: String,
    gesture: Option<Gesture>,
    effect: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Seat {
    pub(crate) names: Vec<String>,
    pub(crate) playing: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Turn {
    pub(crate) description: String,
    pub(crate) player: String,
    pub(crate) column: Option<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Ending {
    pub(crate) score: Option<String>,
    pub(crate) description: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Table {
    pub(crate) columns: Vec<String>,
    pub(crate) history: Vec<Turn>,
    pub(crate) ending: Option<Ending>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Screen {
    board: Board,
    actions: Vec<Action>,
    seat: Seat,
    editable: bool,
    table: Table,
    shown: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum GameSnapshot {
    #[default]
    Loading,
    Error(String),
    Screen(Screen),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Shape {
    Loading,
    Error,
    Screen,
}

impl GameSnapshot {
    fn shape(&self) -> Shape {
        match self {
            Self::Loading => Shape::Loading,
            Self::Error(_) => Shape::Error,
            Self::Screen(_) => Shape::Screen,
        }
    }

    fn as_error(&self) -> String {
        match self {
            Self::Error(error) => error.clone(),
            _ => String::new(),
        }
    }

    fn as_screen(&self) -> Screen {
        match self {
            Self::Screen(screen) => screen.clone(),
            _ => Screen::default(),
        }
    }

    pub(crate) fn screen(
        screen: GameScreen,
        seat: Seat,
        editable: bool,
        table: Table,
        shown: Option<usize>,
    ) -> Self {
        Self::Screen(Screen {
            board: *screen.board,
            actions: screen
                .actions
                .into_iter()
                .enumerate()
                .map(
                    |(
                        index,
                        GameActionOption {
                            label,
                            gesture,
                            effect,
                        },
                    )| Action {
                        index,
                        label,
                        gesture,
                        effect,
                    },
                )
                .collect(),
            seat,
            editable,
            table,
            shown,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Steps {
    game: Stored<Rc<dyn GameModel>>,
    count: Memo<usize>,
    shown: Memo<usize>,
}

impl Steps {
    pub(crate) fn show(&self, shown: usize) {
        let count = self.count.get_untracked();
        self.game
            .value()
            .show_turns((shown < count).then_some(shown));
    }

    pub(crate) fn first(&self) {
        self.show(0);
    }

    pub(crate) fn previous(&self) {
        self.show(self.shown.get_untracked().saturating_sub(1));
    }

    pub(crate) fn next(&self) {
        self.show(self.shown.get_untracked() + 1);
    }

    pub(crate) fn last(&self) {
        self.show(self.count.get_untracked());
    }
}

#[component]
pub(crate) fn Game(
    editor: Editor,
    game: Stored<Rc<dyn GameModel>>,
    snapshot: ReadSignal<GameSnapshot>,
) -> NodeId {
    view! {
        <List spacing=0.0>
            <Keyed
                value={snapshot}
                key={|snapshot: GameSnapshot| snapshot.shape()}
            >|snapshot: ReadSignal<GameSnapshot>|
                <GameShape editor game snapshot @sizing=ItemSize::Percent(100.0) />
            </Keyed>
        </List>
    }
}

#[component]
fn GameShape(
    editor: Editor,
    game: Stored<Rc<dyn GameModel>>,
    snapshot: ReadSignal<GameSnapshot>,
) -> NodeId {
    let shape = snapshot.get_untracked().shape();
    let loading = shape == Shape::Loading;
    let failed = shape == Shape::Error;
    let playing = shape == Shape::Screen;
    view! {
        <List spacing=0.0>
            <Show condition=loading>
                <LoadingPane @sizing=ItemSize::Percent(100.0) label="Loading the game" />
            </Show>
            <Show condition=failed>
                <GameError snapshot={snapshot} />
            </Show>
            <Show condition=playing>
                <GamePlay
                    editor={editor}
                    game={game}
                    snapshot={snapshot}
                    @sizing=ItemSize::Percent(100.0)
                />
            </Show>
        </List>
    }
}

#[component]
fn GameError(snapshot: ReadSignal<GameSnapshot>) -> NodeId {
    let error = create_memo(move || snapshot.get().as_error());
    view! {
        <Card>
            <List spacing=8.0>
                <Text style=TextStyle::HEADING>"Game unavailable"</Text>
                <Text style=TextStyle::PARAGRAPH @test_id={"game.error"}>{error}</Text>
            </List>
        </Card>
    }
}

#[derive(Clone, Copy)]
struct Panel {
    game: Stored<Rc<dyn GameModel>>,
    play: Play,
    controls: Memo<Vec<Action>>,
    editable: Memo<bool>,
    seat: Memo<Seat>,
    table: Memo<Table>,
    shown: Memo<usize>,
    steps: Steps,
}

#[component]
fn GamePlay(
    editor: Editor,
    game: Stored<Rc<dyn GameModel>>,
    snapshot: ReadSignal<GameSnapshot>,
) -> NodeId {
    let screen = create_memo(move || snapshot.get().as_screen());
    let actions = create_memo(move || screen.get().actions);
    let editable = create_memo(move || screen.get().editable);
    let board = create_memo(move || screen.get().board);
    let seat = create_memo(move || screen.get().seat);
    let table = create_memo(move || screen.get().table);
    let count = create_memo(move || table.with(|table| table.history.len()));
    let shown = create_memo(move || screen.get().shown.unwrap_or(count.get()));
    let controls = create_memo(move || {
        actions
            .get()
            .into_iter()
            .filter(|action| matches!(action.gesture, None | Some(Gesture::Control(_))))
            .collect::<Vec<_>>()
    });
    let selected = create_signal(None);
    let choices = create_signal(Vec::new());
    let dragging = create_signal(None);
    let over = create_signal(None);
    create_effect(move || {
        actions.with(|_| {});
        selected.set(None);
        choices.set(Vec::new());
    });
    let play = Play {
        game,
        actions,
        editable,
        selected,
        choices,
        dragging,
        over,
    };
    let drawn = create_memo(move || board.with(Layout::of));
    let hand = create_memo(move || board.with(|board| board.hand.clone()));
    create_effect(move || {
        let size = drawn.with(|layout| layout.size);
        editor.set_intrinsic_size((size.x > 0.0 && size.y > 0.0).then_some(size));
    });
    let world = editor.world();
    editor.keep_fitted(|| Some(Fit::World));
    let layout = create_memo(move || drawn.with(|layout| layout.centered_in(world.get())));
    let steps = Steps { game, count, shown };
    let panel = Panel {
        game,
        play,
        controls,
        editable,
        seat,
        table,
        shown,
        steps,
    };
    let stage = NodeRef::new();
    editor.content(&stage);
    let chrome = editor.chrome_shown();
    view! {
        <List spacing=0.0>
            <Keyed value={chrome} key={|shown: bool| shown}>|chrome: ReadSignal<bool>| {
                match chrome.get_untracked() {
                    true => view! {
                        <List
                            direction=Direction::Horizontal
                            spacing=0.0
                            @sizing=ItemSize::Percent(100.0)
                        >
                            <Stage
                                @sizing=ItemSize::Percent(100.0)
                                content={stage}
                                editor
                                play
                                drawn
                                layout
                                hand
                                steps
                            />
                            <Sidebar @sizing=ItemSize::Fixed(SIDEBAR_WIDTH) panel />
                        </List>
                    },
                    false => view! {
                        <Stage
                            @sizing=ItemSize::Percent(100.0)
                            content={stage}
                            editor
                            play
                            drawn
                            layout
                            hand
                            steps
                        />
                    },
                }
            }</Keyed>
            <Banner anchor={stage} table />
        </List>
    }
}

#[component]
fn Sidebar(panel: Panel) -> NodeId {
    let Panel {
        game,
        play,
        controls,
        editable,
        seat,
        table,
        shown,
        steps,
    } = panel;
    let names = create_memo(move || seat.get().names);
    let seat_keys = create_memo(move || (0..names.get().len()).collect::<Vec<_>>());
    let playing = create_memo(move || Some(seat.get().playing));
    let seated = move |seat: Option<usize>| {
        if let Some(seat) = seat {
            game.value().play_as(seat);
        }
    };
    let choosing = create_memo(move || !play.choices.with(Vec::is_empty));
    view! {
        <Frame fill=ColorRole::Surface>
            <Scroll>
                <Frame padding=PANEL_PADDING>
                    <List spacing=SECTION_SPACING>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Text style=TextStyle::CAPTION>"Playing as"</Text>
                            <Select
                                options={view! {
                                    <ForEach keys={seat_keys}>|seat: usize| {
                                        let label = create_memo(move || {
                                            names.with(|names| {
                                                names.get(seat).cloned().unwrap_or_default()
                                            })
                                        });
                                        view! {
                                            <ChoiceOption label />
                                        }
                                    }</ForEach>
                                }}
                                selected={Binding::new(playing, seated)}
                                label="Playing as"
                                @test_id={"game.player"}
                                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                            />
                        </List>
                        <Show condition={choosing}>
                            <Choices play={play} />
                        </Show>
                        <Separator />
                        <MoveTable table shown steps />
                        <Controls game controls editable />
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}

#[component]
fn Controls(
    game: Stored<Rc<dyn GameModel>>,
    controls: Memo<Vec<Action>>,
    editable: Memo<bool>,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <ForEach keys={controls}>|action: Action|
                <ControlButton game action editable={editable} />
            </ForEach>
        </List>
    }
}

#[component]
fn ControlButton(game: Stored<Rc<dyn GameModel>>, action: Action, editable: Memo<bool>) -> NodeId {
    let disabled = create_memo(move || !editable.get());
    let confirming = create_signal(false);
    let asking = create_memo(move || !confirming.get());
    let glyph = match action.gesture {
        Some(Gesture::Control(Control::Resign)) => ICON_FLAG,
        _ => ICON_CHECK,
    };
    let index = action.index;
    let label = action.label.clone();
    let question = format!("{label}?");
    let confirm = format!("Confirm: {label}");
    let effect = action.effect;
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <Show condition={asking}>
                <Button
                    icon_only=true
                    glyph={glyph}
                    label={label.clone()}
                    disabled={disabled}
                    @test_id={format!("game.control.{index}")}
                    on_click={move || confirming.set(true)}
                />
            </Show>
            <Show condition={confirming}>|| {
                let confirm = confirm.clone(); let effect = effect.clone(); let question = question.clone(); view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                        <Text style=TextStyle::CAPTION>{question}</Text>
                        <Button
                            icon_only=true
                            glyph=ICON_CHECK
                            label={confirm}
                            disabled={disabled}
                            @test_id={format!("game.control.{index}.confirm")}
                            on_click={move || game.value().choose(effect.clone())}
                        />
                        <Button
                            icon_only=true
                            glyph=ICON_X
                            label="Cancel"
                            @test_id={format!("game.control.{index}.cancel")}
                            on_click={move || confirming.set(false)}
                        />
                    </List>
                }
            }</Show>
        </List>
    }
}

#[component]
fn Banner(anchor: NodeRef, table: Memo<Table>) -> NodeId {
    let ending = create_memo(move || table.with(|table| table.ending.clone()));
    let dismissed = create_signal(false);
    create_effect(move || {
        if ending.with(Option::is_none) {
            dismissed.set(false);
        }
    });
    let open = create_memo(move || ending.with(Option::is_some) && !dismissed.get());
    let text = create_memo(move || {
        ending.with(|ending| {
            ending
                .as_ref()
                .map(|ending| match &ending.score {
                    Some(score) => format!("{score}  {}", ending.description),
                    None => ending.description.clone(),
                })
                .unwrap_or_default()
        })
    });
    let color = TextColor::Default;
    view! {
        <unstyled::Floating anchor open={open}>
            <Frame padding=BANNER_MARGIN>
                <Frame
                    fill=ColorRole::SurfaceRaised
                    radius={Radius::custom(8.0)}
                    padding={Sides::symmetric(14.0, 8.0)}
                    border={Border::new(ColorRole::Border).width(1.0)}
                    @test_id={"game.banner"}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                        <Text size=TextSize::Lg weight=Weight::Bold color={color}>{text}</Text>
                        <Button
                            icon_only=true
                            glyph=ICON_X
                            label="Close"
                            size=ButtonSize::Small
                            capture_presses=true
                            @test_id={"game.banner.close"}
                            on_click={move || dismissed.set(true)}
                        />
                    </List>
                </Frame>
            </Frame>
        </unstyled::Floating>
    }
}

#[component]
fn Choices(play: Play) -> NodeId {
    view! {
        <Card>
            <List spacing=8.0>
                <Text>"Which move?"</Text>
                <ChoiceButtons play />
            </List>
        </Card>
    }
}

#[component]
fn ChoiceButtons(play: Play) -> NodeId {
    let choices = play.choices;
    let keys = create_memo(move || choices.get());
    let cancel = move || play.cancel();
    view! {
        <List spacing=6.0>
            <ForEach keys={keys}>|action: Action| {
                let index = action.index;
                view! {
                    <Button
                        label={action.label.clone()}
                        variant=ButtonVariant::Primary
                        @test_id={format!("game.choice.{index}")}
                        on_click={move || play.choose(&action)}
                    />
                }
            }</ForEach>
            <Button
                label="Cancel"
                variant=ButtonVariant::Secondary
                @test_id={"game.choice.cancel"}
                on_click={cancel}
            />
        </List>
    }
}

pub(crate) trait GameCreationModel {
    fn choose_staged(&self, index: usize);
    fn choose_module(&self);
    fn accept(&self);
    fn cancel(&self);
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum StagedState {
    Loading,
    Ready { chosen: bool },
    Failed(String),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct StagedChoice {
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) state: StagedState,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CreationSnapshot {
    pub(crate) games: Vec<StagedChoice>,
    pub(crate) listing: bool,
    pub(crate) module: Option<String>,
    pub(crate) picking: bool,
    pub(crate) error: Option<String>,
    pub(crate) ready: bool,
}

#[component]
pub(crate) fn GameCreation(
    creation: Stored<Rc<dyn GameCreationModel>>,
    snapshot: ReadSignal<CreationSnapshot>,
) -> NodeId {
    let picking = create_memo(move || snapshot.get().picking);
    let games = create_memo(move || snapshot.get().games);
    let status = create_memo(move || {
        let snapshot = snapshot.get();
        if let Some(error) = snapshot.error {
            return error;
        }
        if let Some(module) = snapshot.module {
            return module;
        }
        if snapshot.listing {
            return "Looking for the games this app has".to_owned();
        }
        if snapshot.games.is_empty() {
            return "This app has no games of its own".to_owned();
        }
        String::new()
    });
    let has_status = create_memo(move || !status.get().is_empty());
    let status_color = create_memo(move || match snapshot.get().error.is_some() {
        true => TextColor::Danger,
        false => TextColor::Muted,
    });
    let unready = create_memo(move || !snapshot.get().ready);
    let accepted = create_signal(false);
    view! {
        <DialogBody
            title="New Game"
            id="creation"
            actions={vec![
                DialogAction::cancel("Cancel"),
                DialogAction::primary("Create", move || {
                    accepted.set(true);
                    creation.value().accept();
                })
                .disabled(unready)
                .busy(accepted),
            ]}
            on_dismiss={move || creation.value().cancel()}
        >
            <List spacing=10.0 align=Align::Start>
                <ForEach keys={games}>|game: StagedChoice| {
                    let index = game.index;
                    let (variant, disabled, caption) = match &game.state {
                        StagedState::Loading => (ButtonVariant::Secondary, true, String::new()),
                        StagedState::Ready { chosen: true } => {
                            (ButtonVariant::Primary, false, String::new())
                        }
                        StagedState::Ready { chosen: false } => {
                            (ButtonVariant::Secondary, false, String::new())
                        }
                        StagedState::Failed(error) => (ButtonVariant::Secondary, true, error.clone()),
                    };
                    let failed = !caption.is_empty();
                    view! {
                        <List spacing=2.0 align=Align::Start>
                            <Button
                                label={game.name.clone()}
                                variant
                                disabled
                                @test_id={format!("game.staged.{index}")}
                                on_click={move || creation.value().choose_staged(index)}
                            />
                            <Show condition={failed}>|| {
                                let caption = caption.clone(); view! {
                                    <Text style=TextStyle::CAPTION color=TextColor::Danger>
                                        {caption}
                                    </Text>
                                }
                            }</Show>
                        </List>
                    }
                }</ForEach>
                <Button
                    label="Choose game module…"
                    variant=ButtonVariant::Secondary
                    disabled={picking}
                    @test_id={"game.choose"}
                    on_click={move || creation.value().choose_module()}
                />
                <Show condition={has_status}>
                    <Text style=TextStyle::CAPTION color={status_color} @test_id={"game.selection"}>
                        {status}
                    </Text>
                </Show>
            </List>
        </DialogBody>
    }
}
