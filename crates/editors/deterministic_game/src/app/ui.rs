use std::rc::Rc;

use block_editor_beui::Editor;
use block_editor_beui::beui::icons::{ICON_CHECK, ICON_CLOSE, ICON_FLAG};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, Keyed, List, Memo, NodeRef, ReadSignal, Show, Text,
    clone, component, create_effect, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Heading, IconButton, IconButtonSize, Paragraph,
    Scroll, Select, Separator, use_theme,
};
use block_editor_beui::beui::unstyled::{self, ChoiceOption};
use block_editor_beui::beui::{NodeId, TextAlign};
use game_api::{Board, Control, GameActionOption, GameScreen, Gesture};

mod annotations;
mod board;
mod layout;
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
const BANNER_SIZE: f32 = 17.0;
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

#[derive(Clone)]
pub(crate) struct Steps {
    game: Rc<dyn GameModel>,
    count: Memo<usize>,
    shown: Memo<usize>,
}

impl Steps {
    pub(crate) fn show(&self, shown: usize) {
        let count = self.count.get_untracked();
        self.game.show_turns((shown < count).then_some(shown));
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
    game: Rc<dyn GameModel>,
    snapshot: ReadSignal<GameSnapshot>,
) -> NodeId {
    view! {
        <List spacing=0.0>
            <Keyed value={snapshot} key={|snapshot: GameSnapshot| snapshot.shape()}>
                {move |snapshot: ReadSignal<GameSnapshot>| {
                    let (editor, game) = (editor.clone(), game.clone());
                    view! {
                        <GameShape editor game snapshot @sizing=ItemSize::Percent(100.0) />
                    }
                }}
            </Keyed>
        </List>
    }
}

#[component]
fn GameShape(
    editor: Editor,
    game: Rc<dyn GameModel>,
    snapshot: ReadSignal<GameSnapshot>,
) -> NodeId {
    let shape = snapshot.get_untracked().shape();
    let loading = shape == Shape::Loading;
    let failed = shape == Shape::Error;
    let playing = shape == Shape::Screen;
    let failure = snapshot.clone();
    let theme = use_theme();
    view! {
        <List spacing=0.0>
            <Show condition=loading>
                <Frame
                    color={theme.surface.clone()}
                    padding_horizontal=PANEL_PADDING
                    padding_vertical=PANEL_PADDING
                >
                    <Body content="Loading game..." align=TextAlign::Center />
                </Frame>
            </Show>
            <Show condition=failed>
                <GameError snapshot={failure} />
            </Show>
            <Show condition=playing>
                <GamePlay editor game snapshot @sizing=ItemSize::Percent(100.0) />
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
                <Heading content="Game unavailable" />
                <Paragraph content={error} @test_id={"game.error"} />
            </List>
        </Card>
    }
}

#[derive(Clone)]
struct Panel {
    game: Rc<dyn GameModel>,
    play: Play,
    controls: Memo<Vec<Action>>,
    editable: Memo<bool>,
    seat: Memo<Seat>,
    table: Memo<Table>,
    shown: Memo<usize>,
    steps: Steps,
}

#[component]
fn GamePlay(editor: Editor, game: Rc<dyn GameModel>, snapshot: ReadSignal<GameSnapshot>) -> NodeId {
    let screen = create_memo(move || snapshot.get().as_screen());
    let actions = create_memo(clone!(screen -> move || screen.get().actions));
    let editable = create_memo(clone!(screen -> move || screen.get().editable));
    let board = create_memo(clone!(screen -> move || screen.get().board));
    let seat = create_memo(clone!(screen -> move || screen.get().seat));
    let table = create_memo(clone!(screen -> move || screen.get().table));
    let count = create_memo(clone!(table -> move || table.with(|table| table.history.len())));
    let shown =
        create_memo(clone!(screen count -> move || screen.get().shown.unwrap_or(count.get())));
    let controls = create_memo(clone!(actions -> move || {
        actions
            .get()
            .into_iter()
            .filter(|action| matches!(action.gesture, None | Some(Gesture::Control(_))))
            .collect::<Vec<_>>()
    }));
    let (selected, set_selected) = create_signal(None);
    let (choices, set_choices) = create_signal(Vec::new());
    let (dragging, set_dragging) = create_signal(None);
    let (over, set_over) = create_signal(None);
    create_effect(clone!(actions set_selected set_choices -> move || {
        actions.with(|_| {});
        set_selected.set(None);
        set_choices.set(Vec::new());
    }));
    let play = Play {
        game: game.clone(),
        actions,
        editable: editable.clone(),
        selected,
        set_selected,
        choices,
        set_choices,
        dragging,
        set_dragging,
        over,
        set_over,
    };
    let drawn = create_memo(clone!(board -> move || board.with(Layout::of)));
    let sized = editor.clone();
    create_effect(clone!(drawn -> move || {
        let size = drawn.with(|layout| layout.size);
        sized.set_intrinsic_size((size.x > 0.0 && size.y > 0.0).then_some(size));
    }));
    let world = editor.world();
    let layout = create_memo(move || drawn.with(|layout| layout.centered_in(world.get())));
    let steps = Steps {
        game: game.clone(),
        count,
        shown: shown.clone(),
    };
    let panel = Panel {
        game,
        play: play.clone(),
        controls,
        editable,
        seat,
        table: table.clone(),
        shown,
        steps: steps.clone(),
    };
    let stage = NodeRef::new();
    editor.content(&stage);
    let chrome = editor.chrome_shown();
    let banner_anchor = stage.clone();
    view! {
        <List spacing=0.0>
            <Keyed value={chrome} key={|shown: bool| shown}>
                {move |chrome: ReadSignal<bool>| {
                    let stage = stage.clone();
                    let (editor, play, layout, steps, panel) = (
                        editor.clone(),
                        play.clone(),
                        layout.clone(),
                        steps.clone(),
                        panel.clone(),
                    );
                    match chrome.get_untracked() {
                        true => view! {
                            <List
                                direction=Direction::Horizontal
                                spacing=0.0
                                @sizing=ItemSize::Percent(100.0)
                            >
                                <Stage
                                    @sizing=ItemSize::Percent(100.0)
                                    @node_ref={&stage}
                                    editor
                                    play
                                    layout
                                    steps
                                />
                                <Sidebar @sizing=ItemSize::Fixed(SIDEBAR_WIDTH) panel />
                            </List>
                        },
                        false => view! {
                            <List spacing=0.0 @sizing=ItemSize::Percent(100.0)>
                                <Stage
                                    @sizing=ItemSize::Percent(100.0)
                                    @node_ref={&stage}
                                    editor
                                    play
                                    layout
                                    steps
                                />
                                <Bar panel />
                            </List>
                        },
                    }
                }}
            </Keyed>
            <Banner anchor={banner_anchor} table />
        </List>
    }
}

#[component]
fn Sidebar(panel: Panel) -> NodeId {
    let theme = use_theme();
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
    let names = create_memo(clone!(seat -> move || seat.get().names));
    let seat_keys =
        create_memo(clone!(names -> move || (0..names.get().len()).collect::<Vec<_>>()));
    let playing = create_memo(clone!(seat -> move || Some(seat.get().playing)));
    let seated = clone!(game -> move |seat: Option<usize>| {
        if let Some(seat) = seat {
            game.play_as(seat);
        }
    });
    let choosing = create_memo(clone!(play -> move || !play.choices.with(Vec::is_empty)));
    let choosing_play = play.clone();
    view! {
        <Frame color={theme.surface.clone()}>
            <Scroll>
                <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
                    <List spacing=SECTION_SPACING>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Caption content="Playing as" />
                            <Select
                                options={view! {
                                    <ForEach keys={seat_keys}>
                                        {move |seat: usize| {
                                            let label = create_memo(clone!(names -> move || {
                                                names.with(|names| {
                                                    names.get(seat).cloned().unwrap_or_default()
                                                })
                                            }));
                                            view! {
                                                <ChoiceOption label />
                                            }
                                        }}
                                    </ForEach>
                                }}
                                selected={playing}
                                label="Playing as"
                                @test_id={"game.player"}
                                on_change={seated}
                            />
                        </List>
                        <Show condition={choosing}>
                            <Choices play={choosing_play} />
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
fn Bar(panel: Panel) -> NodeId {
    let theme = use_theme();
    let Panel {
        game,
        play,
        controls,
        editable,
        ..
    } = panel;
    let choosing = create_memo(clone!(play -> move || !play.choices.with(Vec::is_empty)));
    let idle = create_memo(clone!(choosing -> move || !choosing.get()));
    let needed = create_memo(clone!(choosing controls -> move || {
        choosing.get() || !controls.with(Vec::is_empty)
    }));
    view! {
        <List spacing=0.0>
            <Show condition={needed}>
                <Frame color={theme.surface.clone()} padding_horizontal=10.0 padding_vertical=6.0>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Show condition={choosing}>
                            <ChoiceButtons play />
                        </Show>
                        <Show condition={idle}>
                            <Controls game controls editable />
                        </Show>
                    </List>
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn Controls(game: Rc<dyn GameModel>, controls: Memo<Vec<Action>>, editable: Memo<bool>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <ForEach keys={controls}>
                {move |action: Action| {
                    let game = game.clone();
                    view! {
                        <ControlButton game action editable={editable.clone()} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn ControlButton(game: Rc<dyn GameModel>, action: Action, editable: Memo<bool>) -> NodeId {
    let disabled = create_memo(move || !editable.get());
    let (confirming, set_confirming) = create_signal(false);
    let asking = create_memo(clone!(confirming -> move || !confirming.get()));
    let glyph = match action.gesture {
        Some(Gesture::Control(Control::Resign)) => ICON_FLAG,
        _ => ICON_CHECK,
    };
    let index = action.index;
    let label = action.label.clone();
    let question = format!("{label}?");
    let confirm = format!("Confirm: {label}");
    let effect = action.effect;
    let (ask, cancel) = (set_confirming.clone(), set_confirming.clone());
    let asking_disabled = disabled.clone();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <Show condition={asking}>
                <IconButton
                    glyph={glyph.to_owned()}
                    label={label}
                    disabled={asking_disabled}
                    @test_id={format!("game.control.{index}")}
                    on_click={move || ask.set(true)}
                />
            </Show>
            <Show condition={confirming}>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                    <Caption content={question} />
                    <IconButton
                        glyph={ICON_CHECK.to_owned()}
                        label={confirm}
                        disabled={disabled}
                        @test_id={format!("game.control.{index}.confirm")}
                        on_click={move || game.choose(effect.clone())}
                    />
                    <IconButton
                        glyph={ICON_CLOSE.to_owned()}
                        label="Cancel"
                        @test_id={format!("game.control.{index}.cancel")}
                        on_click={move || cancel.set(false)}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn Banner(anchor: NodeRef, table: Memo<Table>) -> NodeId {
    let ending = create_memo(move || table.with(|table| table.ending.clone()));
    let (dismissed, set_dismissed) = create_signal(false);
    create_effect(clone!(ending set_dismissed -> move || {
        if ending.with(Option::is_none) {
            set_dismissed.set(false);
        }
    }));
    let open = create_memo(clone!(ending dismissed -> move || {
        ending.with(Option::is_some) && !dismissed.get()
    }));
    let text = create_memo(clone!(ending -> move || {
        ending.with(|ending| {
            ending
                .as_ref()
                .map(|ending| match &ending.score {
                    Some(score) => format!("{score}  {}", ending.description),
                    None => ending.description.clone(),
                })
                .unwrap_or_default()
        })
    }));
    let theme = use_theme();
    let color = create_memo(clone!(theme -> move || theme.get().text));
    view! {
        <unstyled::Floating anchor open={open}>
            <Frame padding_horizontal=BANNER_MARGIN padding_vertical=BANNER_MARGIN>
                <Frame
                    color={theme.surface_raised.clone()}
                    radius=8
                    padding_horizontal=14.0
                    padding_vertical=8.0
                    outline={theme.border.clone()}
                    outline_width=1.0
                    outline_visible=true
                    @test_id={"game.banner"}
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                        <Text string={text} font_size=BANNER_SIZE bold=true color={color} />
                        <IconButton
                            glyph={ICON_CLOSE.to_owned()}
                            label="Close"
                            size=IconButtonSize::Compact
                            capture_presses=true
                            @test_id={"game.banner.close"}
                            on_click={move || set_dismissed.set(true)}
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
                <Body content="Which move?" />
                <ChoiceButtons play />
            </List>
        </Card>
    }
}

#[component]
fn ChoiceButtons(play: Play) -> NodeId {
    let choices = play.choices.clone();
    let keys = create_memo(move || choices.get());
    let cancel = clone!(play -> move || play.cancel());
    view! {
        <List spacing=6.0>
            <ForEach keys={keys}>
                {move |action: Action| {
                    let play = play.clone();
                    let index = action.index;
                    view! {
                        <Button
                            label={action.label.clone()}
                            variant=ButtonVariant::Primary
                            @test_id={format!("game.choice.{index}")}
                            on_click={move || play.choose(&action)}
                        />
                    }
                }}
            </ForEach>
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
    fn choose_module(&self);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CreationSnapshot {
    pub(crate) chosen: Option<String>,
    pub(crate) picking: bool,
    pub(crate) error: Option<String>,
}

#[component]
pub(crate) fn GameCreation(
    creation: Rc<dyn GameCreationModel>,
    snapshot: ReadSignal<CreationSnapshot>,
) -> NodeId {
    let picking = create_memo(clone!(snapshot -> move || snapshot.get().picking));
    let status = create_memo(clone!(snapshot -> move || {
        let snapshot = snapshot.get();
        snapshot.error.unwrap_or_else(|| {
            snapshot
                .chosen
                .unwrap_or_else(|| "No game module chosen".to_owned())
        })
    }));
    let theme = use_theme();
    let status_color = create_memo(clone!(theme snapshot -> move || {
        let theme = theme.get();
        if snapshot.get().error.is_some() {
            theme.danger
        } else {
            theme.text_muted
        }
    }));
    view! {
        <Frame padding_horizontal=14.0 padding_vertical=14.0>
            <List spacing=10.0 align=Align::Start>
                <Button
                    label="Choose game module…"
                    variant=ButtonVariant::Primary
                    disabled={picking}
                    @test_id={"game.choose"}
                    on_click={move || creation.choose_module()}
                />
                <Caption content={status} color={status_color} @test_id={"game.selection"} />
            </List>
        </Frame>
    }
}
