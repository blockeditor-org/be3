use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    Column, ForEach, Frame, ItemSize, Keyed, ReadSignal, Scroll, clone, component, create_memo,
    percent, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Card, Heading, Paragraph, use_theme,
};
use block_editor_plugin::beui::{NodeId, TextAlign};
use game_api::{GameActionOption, GameScreen};

const PAGE_PADDING: f32 = 24.0;

pub(crate) trait GameModel {
    fn choose(&self, effect: Vec<u8>);
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
struct Action {
    index: usize,
    label: String,
    effect: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Screen {
    description: String,
    actions: Vec<Action>,
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
    pub(crate) fn screen(screen: GameScreen, editable: bool) -> Self {
        Self::Screen(Screen {
            description: screen.description,
            actions: screen
                .actions
                .into_iter()
                .enumerate()
                .map(|(index, GameActionOption { label, effect })| Action {
                    index,
                    label,
                    effect,
                })
                .collect(),
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
            <Keyed value={snapshot} key={|snapshot: GameSnapshot| snapshot.shape()}>
                {move |snapshot: ReadSignal<GameSnapshot>| {
                    percent(game_view(game.clone(), snapshot), 100.0)
                }}
            </Keyed>
        </Frame>
    }
}

fn game_view(game: Rc<dyn GameModel>, snapshot: ReadSignal<GameSnapshot>) -> NodeId {
    match snapshot.get_untracked().shape() {
        Shape::Loading => view! {
            <Body content="Loading game..." align=TextAlign::Center />
        },
        Shape::Error => {
            let error = create_memo(move || snapshot.get().as_error());
            view! {
                <Card>
                    <Column spacing=8.0>
                        <Heading content="Game unavailable" />
                        <Paragraph content={error} @test_id={"game.error"} />
                    </Column>
                </Card>
            }
        }
        Shape::Screen => {
            let screen = create_memo(move || snapshot.get().as_screen());
            let description = create_memo(clone!(screen -> move || screen.get().description));
            let actions = create_memo(clone!(screen -> move || screen.get().actions));
            let editable = create_memo(clone!(screen -> move || screen.get().editable));
            view! {
                <Column spacing=16.0>
                    <Heading content={description} />
                    <Scroll @sizing=ItemSize::Percent(100.0)>
                        <ForEach spacing=10.0 keys={actions}>
                            {move |action: Action| {
                                let effect = action.effect;
                                let game = game.clone();
                                let disabled = create_memo(
                                    clone!(editable -> move || !editable.get()),
                                );
                                view! {
                                    <Button
                                        label={action.label}
                                        variant=ButtonVariant::Primary
                                        disabled={disabled}
                                        @test_id={format!("game.action.{}", action.index)}
                                        on_click={move || game.choose(effect.clone())}
                                    />
                                }
                            }}
                        </ForEach>
                    </Scroll>
                </Column>
            }
        }
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
            theme.accent_hover
        } else {
            theme.text_muted
        }
    }));
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=12.0 padding_vertical=10.0>
            <Column spacing=8.0>
                <Button
                    label="Choose game module..."
                    variant=ButtonVariant::Secondary
                    disabled={picking}
                    @test_id={"game.choose"}
                    on_click={move || creation.choose_module()}
                />
                <Body content={status} color={status_color} @test_id={"game.selection"} />
            </Column>
        </Frame>
    }
}
