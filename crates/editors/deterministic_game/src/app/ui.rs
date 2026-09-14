use std::rc::Rc;

use block_editor_plugin::beui::reactive::{
    build, clone, create_memo, create_signal, view, with_reactive_scope, Column, Dynamic, ForEach,
    Frame, ItemSize, Scroll, WriteSignal,
};
use block_editor_plugin::beui::styled::{
    use_theme, Body, Button, ButtonVariant, Card, Heading, Paragraph,
};
use block_editor_plugin::beui::{Color32, Context, Document, NodeId, Rect, TextAlign};
use game_api::{GameActionOption, GameScreen};

const PAGE_PADDING: f32 = 24.0;

pub(super) trait GameModel {
    fn choose(&self, effect: Vec<u8>);
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
struct Action {
    index: usize,
    label: String,
    effect: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Screen {
    description: String,
    actions: Vec<Action>,
    editable: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) enum GameSnapshot {
    #[default]
    Loading,
    Error(String),
    Screen(Screen),
}

impl GameSnapshot {
    pub(super) fn screen(screen: GameScreen, editable: bool) -> Self {
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

pub struct GameUi {
    document: Document,
    set_snapshot: WriteSignal<GameSnapshot>,
}

impl GameUi {
    pub(super) fn new(game: Rc<dyn GameModel>, initial: GameSnapshot) -> Self {
        let (snapshot, set_snapshot) = create_signal(initial);
        let document = build(move || {
            let theme = use_theme();
            view! {
                <Frame color={theme.pick(|theme| theme.background)} padding_horizontal=PAGE_PADDING padding_vertical=PAGE_PADDING>
                    <Dynamic value={snapshot} item_size=ItemSize::Percent(100.0)>
                        {move |snapshot| game_view(game.clone(), snapshot)}
                    </Dynamic>
                </Frame>
            }
        });
        Self {
            document,
            set_snapshot,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn background(&self) -> Color32 {
        self.document.theme().background
    }

    pub(super) fn set_snapshot(&mut self, snapshot: GameSnapshot) {
        let set_snapshot = self.set_snapshot.clone();
        with_reactive_scope(&mut self.document, move || set_snapshot.set(snapshot));
    }

    pub(super) fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}

fn game_view(game: Rc<dyn GameModel>, snapshot: GameSnapshot) -> NodeId {
    match snapshot {
        GameSnapshot::Loading => view! {
            <Body content="Loading game..." align=TextAlign::Center />
        },
        GameSnapshot::Error(error) => view! {
            <Card>
                <Column spacing=8.0>
                    <Heading content="Game unavailable" />
                    <Paragraph content={error} @test_id={"game.error"} />
                </Column>
            </Card>
        },
        GameSnapshot::Screen(screen) => {
            let actions = screen.actions;
            let editable = screen.editable;
            view! {
                <Column spacing=16.0>
                    <Heading content={screen.description} />
                    <Scroll @sizing=ItemSize::Percent(100.0)>
                        <ForEach
                            spacing=10.0
                            items={actions}
                            key={|action: Action| action.index}
                        >
                            {move |action: Action| {
                                let effect = action.effect;
                                let game = game.clone();
                                view! {
                                    <Button
                                        label={action.label}
                                        variant=ButtonVariant::Primary
                                        disabled={!editable}
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

pub(super) trait GameCreationModel {
    fn choose_module(&self);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CreationSnapshot {
    pub(super) chosen: Option<String>,
    pub(super) picking: bool,
    pub(super) error: Option<String>,
}

pub struct GameCreationUi {
    document: Document,
    set_snapshot: WriteSignal<CreationSnapshot>,
}

impl GameCreationUi {
    pub(super) fn new(creation: Rc<dyn GameCreationModel>) -> Self {
        let (snapshot, set_snapshot) = create_signal(CreationSnapshot::default());
        let document = build(move || {
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
            let status_color = create_memo(clone!(theme -> move || {
                let theme = theme.get();
                if snapshot.get().error.is_some() {
                    theme.accent_hover
                } else {
                    theme.text_muted
                }
            }));
            view! {
                <Frame color={theme.pick(|theme| theme.background)} padding_horizontal=12.0 padding_vertical=10.0>
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
        });
        Self {
            document,
            set_snapshot,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub(super) fn set_snapshot(&mut self, snapshot: CreationSnapshot) {
        let set_snapshot = self.set_snapshot.clone();
        with_reactive_scope(&mut self.document, move || set_snapshot.set(snapshot));
    }

    pub(super) fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}
