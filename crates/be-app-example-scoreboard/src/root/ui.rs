use std::collections::HashMap;

use be_sdk_beui::be_block::ObjectId;
use be_sdk_beui::beui::icons::{ICON_MINUS, ICON_PLUS, ICON_ROTATE_CCW, ICON_X};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign};
use be_sdk_beui::{ContentProjection, Editor};
use be_type_example_scoreboard::{Player, Scoreboard as ScoreboardModel, ScoreboardContent};
use beui::{Align, Direction, ItemSize};

const COLUMN: Length = Length::Size(Size::Xl);

type Board = ContentProjection<ScoreboardContent>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Order {
    Standings,
    Joined,
}

impl Order {
    const ALL: [Order; 2] = [Order::Standings, Order::Joined];
}

#[component]
pub fn Scoreboard(editor: Editor) -> NodeId {
    let board: Board = editor.block_content::<ScoreboardContent>();
    let joined = board.ids(ObjectId::ROOT, ScoreboardModel::PLAYERS);
    let standings = board.project(|board| board.root().standings());
    let leader = board.project(|board| {
        let board = board.root();
        let first = board.standings().first().copied()?;
        let player = board.players.iter().find(|player| player.id == first)?;
        Some((player.name.clone(), player.score.get()))
    });
    let any_points = board.project(|board| {
        board
            .root()
            .players
            .iter()
            .any(|player| player.score.get() != 0)
    });

    let order = create_signal(Order::Standings);
    let order_index = create_memo(move || {
        Order::ALL
            .iter()
            .position(|shown| *shown == order.get())
            .unwrap_or(0)
    });
    let shown = create_memo(move || match order.get() {
        Order::Standings => standings.get(),
        Order::Joined => joined.get(),
    });
    let places = create_memo(move || {
        standings.with(|standings| {
            standings
                .iter()
                .enumerate()
                .map(|(place, id)| (*id, place + 1))
                .collect::<HashMap<ObjectId, usize>>()
        })
    });
    let summary = create_memo(move || match (joined.with(Vec::len), leader.get()) {
        (0, _) | (_, None) => "Add the first player below".to_owned(),
        (1, Some((name, score))) => format!("{name} is playing alone with {score}"),
        (count, Some((name, score))) => format!("{count} players, {name} leads with {score}"),
    });
    let empty = create_memo(move || joined.with(Vec::is_empty));
    let reset_disabled = create_memo(move || !any_points.get());
    let draft = create_signal(String::new());

    let rows = move |id: ObjectId| {
        let place =
            create_memo(move || places.with(|places| places.get(&id).copied().unwrap_or(0)));
        view! {
            <PlayerRow board id place />
        }
    };

    view! {
        <Frame padding=Space::Xl3>
            <List spacing=Space::Xl>
                <List spacing=Space::Sm>
                    <Text style=TextStyle::HEADING>"Scoreboard"</Text>
                    <Text style=TextStyle::CAPTION @test_id={"scoreboard.summary"}>{summary}</Text>
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={draft}
                        placeholder="Player name"
                        label="New player"
                        @test_id={"scoreboard.draft"}
                        on_submit={move |name| add_player(&board, draft, name)}
                    />
                    <Button
                        label="Add player"
                        glyph={Some(ICON_PLUS)}
                        variant=ButtonVariant::Primary
                        @test_id={"scoreboard.add"}
                        on_click={move || add_player(&board, draft, draft.get())}
                    />
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <SegmentedControl
                        @sizing=ItemSize::Percent(100.0)
                        label="Order"
                        options={view! {
                            <ChoiceOption label="Standings" />
                            <ChoiceOption label="Joined" />
                        }}
                        selected={Binding::new(order_index, move |index: usize| {
                            if let Some(chosen) = Order::ALL.get(index) {
                                order.set(*chosen);
                            }
                        })}
                    />
                    <Button
                        label="Reset scores"
                        glyph={Some(ICON_ROTATE_CCW)}
                        disabled={reset_disabled}
                        @test_id={"scoreboard.reset"}
                        on_click={move || {
                            if let Ok(edit) = board.read(|board| board.root().reset()) {
                                board.operate(edit);
                            }
                        }}
                    />
                </List>
                <Card @sizing=ItemSize::Percent(100.0)>
                    <List spacing=Space::Md>
                        <Show condition={empty}>
                            <Text align=TextAlign::Center color=TextColor::Muted>
                                "No players yet."
                            </Text>
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0)>
                            <List spacing=Space::Sm>
                                <ForEach keys={shown} view={rows} />
                            </List>
                        </Scroll>
                    </List>
                </Card>
            </List>
        </Frame>
    }
}

fn add_player(board: &Board, draft: Signal<String>, name: String) {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return;
    }
    board.operate(ScoreboardModel::add(name).1);
    draft.set(String::new());
}

#[component]
fn PlayerRow(board: Board, id: ObjectId, place: Memo<usize>) -> NodeId {
    let name = board.field(id, Player::NAME);
    let score = board.field(id, Player::SCORE);
    let place = create_memo(move || format!("#{}", place.get()));
    let score_label = create_memo(move || score.get().get().to_string());
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            radius=Radius::Md
            padding={Padding::symmetric(Space::Lg, Space::Sm)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Frame width=COLUMN>
                    <Text color=TextColor::Muted @test_id={format!("scoreboard.player.{id}.place")}>
                        {place}
                    </Text>
                </Frame>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    plain=true
                    label="Player name"
                    value={Binding::new(name, move |name: String| board.operate(ScoreboardModel::rename(id, name)))}
                    @test_id={format!("scoreboard.player.{id}.name")}
                />
                <Button
                    label="Take a point"
                    glyph={Some(ICON_MINUS)}
                    icon_only=true
                    @test_id={format!("scoreboard.player.{id}.minus")}
                    on_click={move || board.operate(ScoreboardModel::score(id, -1))}
                />
                <Frame width=COLUMN>
                    <Text
                        size=TextSize::Lg
                        align=TextAlign::Center
                        @test_id={format!("scoreboard.player.{id}.score")}
                    >
                        {score_label}
                    </Text>
                </Frame>
                <Button
                    label="Give a point"
                    glyph={Some(ICON_PLUS)}
                    icon_only=true
                    variant=ButtonVariant::Primary
                    @test_id={format!("scoreboard.player.{id}.plus")}
                    on_click={move || board.operate(ScoreboardModel::score(id, 1))}
                />
                <Button
                    label="Remove player"
                    glyph={Some(ICON_X)}
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    @test_id={format!("scoreboard.player.{id}.remove")}
                    on_click={move || board.operate(ScoreboardModel::remove(id))}
                />
            </List>
        </Frame>
    }
}
