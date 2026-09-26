use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, Keyed, List, Memo, ReadSignal, Show, clone,
    component, create_effect, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Heading, Paragraph, Scroll, Select, use_theme,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, TextAlign};
use game_api::{Board, GameActionOption, GameScreen, Gesture};

mod board;
mod play;
mod skin;

use board::Board as BoardView;
use play::Play;

const PAGE_PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 16.0;

pub(crate) trait GameModel {
    fn choose(&self, effect: Vec<u8>);
    fn play_as(&self, seat: usize);
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
pub(crate) struct Screen {
    description: String,
    board: Board,
    actions: Vec<Action>,
    seat: Seat,
    editable: bool,
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
}

impl GameSnapshot {
    pub(crate) fn screen(screen: GameScreen, seat: Seat, editable: bool) -> Self {
        Self::Screen(Screen {
            description: screen.description,
            board: screen.board,
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
        })
    }
}

#[component]
pub(crate) fn Game(game: Rc<dyn GameModel>, snapshot: ReadSignal<GameSnapshot>) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=PAGE_PADDING
            padding_vertical=PAGE_PADDING
        >
            <List spacing=0.0>
                <Keyed value={snapshot} key={|snapshot: GameSnapshot| snapshot.shape()}>
                    {move |snapshot: ReadSignal<GameSnapshot>| {
                        let game = game.clone();
                        view! {
                            <GameShape game snapshot @sizing=ItemSize::Percent(100.0) />
                        }
                    }}
                </Keyed>
            </List>
        </Frame>
    }
}

#[component]
fn GameShape(game: Rc<dyn GameModel>, snapshot: ReadSignal<GameSnapshot>) -> NodeId {
    let shape = snapshot.get_untracked().shape();
    let loading = shape == Shape::Loading;
    let failed = shape == Shape::Error;
    let playing = shape == Shape::Screen;
    let failure = snapshot.clone();
    view! {
        <List spacing=0.0>
            <Show condition=loading>
                <Body content="Loading game..." align=TextAlign::Center />
            </Show>
            <Show condition=failed>
                <GameError snapshot={failure} />
            </Show>
            <Show condition=playing>
                <GamePlay game snapshot @sizing=ItemSize::Percent(100.0) />
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

#[component]
fn GamePlay(game: Rc<dyn GameModel>, snapshot: ReadSignal<GameSnapshot>) -> NodeId {
    let screen = create_memo(move || snapshot.get().as_screen());
    let description = create_memo(clone!(screen -> move || screen.get().description));
    let actions = create_memo(clone!(screen -> move || screen.get().actions));
    let editable = create_memo(clone!(screen -> move || screen.get().editable));
    let board = create_memo(clone!(screen -> move || screen.get().board));
    let seat = create_memo(clone!(screen -> move || screen.get().seat));
    let names = create_memo(clone!(seat -> move || seat.get().names));
    let seat_keys =
        create_memo(clone!(names -> move || (0..names.get().len()).collect::<Vec<_>>()));
    let playing = create_memo(clone!(seat -> move || Some(seat.get().playing)));
    let buttons = create_memo(clone!(actions -> move || {
        actions
            .get()
            .into_iter()
            .filter(|action| action.gesture.is_none())
            .collect::<Vec<_>>()
    }));
    let (selected, set_selected) = create_signal(None);
    let (choices, set_choices) = create_signal(Vec::new());
    create_effect(clone!(actions set_selected set_choices -> move || {
        actions.with(|_| {});
        set_selected.set(None);
        set_choices.set(Vec::new());
    }));
    let play = Play {
        game: game.clone(),
        actions,
        editable: editable.clone(),
        board: board.clone(),
        selected,
        set_selected,
        choices: choices.clone(),
        set_choices,
    };
    let choosing_play = play.clone();
    let choosing = create_memo(clone!(choices -> move || !choices.with(Vec::is_empty)));
    let seated = clone!(game -> move |seat: Option<usize>| {
        if let Some(seat) = seat {
            game.play_as(seat);
        }
    });
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Caption content="Playing as" />
                <Select
                    options={view! {
                        <ForEach keys={seat_keys}>
                            {move |seat: usize| {
                                let label = create_memo(clone!(names -> move || {
                                    names.with(|names| names.get(seat).cloned().unwrap_or_default())
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
            <Heading content={description} @test_id={"game.description"} />
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <List spacing=SECTION_SPACING>
                    <Show condition={choosing}>
                        <Choices play={choosing_play} />
                    </Show>
                    <BoardView play={play.clone()} board />
                    <List spacing=10.0>
                        <ForEach keys={buttons}>
                            {move |action: Action| {
                                let game = game.clone();
                                view! {
                                    <ActionButton game action editable={editable.clone()} />
                                }
                            }}
                        </ForEach>
                    </List>
                </List>
            </Scroll>
        </List>
    }
}

#[component]
fn ActionButton(game: Rc<dyn GameModel>, action: Action, editable: Memo<bool>) -> NodeId {
    let effect = action.effect;
    let disabled = create_memo(move || !editable.get());
    view! {
        <Button
            label={action.label}
            variant=ButtonVariant::Primary
            disabled={disabled}
            @test_id={format!("game.action.{}", action.index)}
            on_click={move || game.choose(effect.clone())}
        />
    }
}

#[component]
fn Choices(play: Play) -> NodeId {
    let choices = play.choices.clone();
    let keys = create_memo(move || choices.get());
    let cancel = clone!(play -> move || play.cancel());
    view! {
        <Card>
            <List spacing=8.0>
                <Body content="Which move?" />
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
        </Card>
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
