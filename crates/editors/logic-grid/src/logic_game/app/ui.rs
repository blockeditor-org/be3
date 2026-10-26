use uuid::Uuid;

use block_editor_beui::Editor;
use block_editor_beui::beui::icons::{ICON_BLOCKS, ICON_CIRCLE_CHECK, ICON_PLUS, ICON_TRASH};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Vec2};
use logicgame::challenges::{ChallengeId, generate_challenge};

use crate::logic_game::binary_addition::ui::BinaryAddition;

use super::game::{Game, Level, Solution};
use block_editor_beui::beui::TextAlign;

const PADDING: f32 = 16.0;
const ROW_SPACING: f32 = 8.0;
const INDENT: f32 = 20.0;
const ROW_HEIGHT: f32 = 26.0;
const CHROME_HEIGHT: f32 = 120.0;
const QUIZ_HEIGHT: f32 = 320.0;
const INTRINSIC_WIDTH: f32 = 720.0;

#[component]
pub fn LogicGameEditor(editor: Editor) -> NodeId {
    let block = editor.block_content::<logic_grid_types::LogicGameContent>();
    let game = Game::watch(editor, block);
    let levels = game.levels();
    let hotbar = game.hotbar();
    let expanded = create_signal(None::<ChallengeId>);

    create_effect(move || {
        let open = expanded.get();
        let rows = levels.with(|levels| {
            levels.len()
                + levels
                    .iter()
                    .filter(|level| Some(level.challenge) == open)
                    .map(|level| level.solutions.len() + 1)
                    .sum::<usize>()
        });
        let quiz = match open == Some(ChallengeId::BinaryAddition) {
            true => QUIZ_HEIGHT,
            false => 0.0,
        };
        editor.set_intrinsic_size(Some(Vec2::new(
            INTRINSIC_WIDTH,
            CHROME_HEIGHT + ROW_HEIGHT * rows as f32 + quiz,
        )));
    });

    let has_hotbar = create_memo(move || hotbar.get().is_some());
    let open_hotbar = move || game.open_hotbar();
    let keys = create_memo(move || {
        levels.with(|levels| {
            levels
                .iter()
                .map(|level| level.challenge)
                .collect::<Vec<_>>()
        })
    });
    let open = create_selector(move || expanded.get());

    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <Frame @node_ref={&content} padding=PADDING>
            <List spacing=ROW_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Text style=TextStyle::HEADING>"Levels"</Text>
                    <Show condition={has_hotbar}>
                        <Button
                            icon_only=true
                            glyph=ICON_BLOCKS
                            label="Hotbar"
                            @test_id={"logic-game.hotbar"}
                            on_click={open_hotbar}
                        />
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=4.0>
                        <ForEach keys={keys}>|challenge: ChallengeId| {
                            let level = level_of(levels, challenge);
                            let here = open.memo(Some(challenge));
                            let toggle = move || {
                                let shown = expanded.get_untracked() == Some(challenge);
                                expanded.set((!shown).then_some(challenge));
                            };
                            view! {
                                <LevelRow
                                    editor={editor}
                                    block={block}
                                    game={game}
                                    challenge={challenge}
                                    level={level}
                                    expanded={here}
                                    on_toggle={toggle}
                                />
                            }
                        }</ForEach>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn LevelRow(
    editor: Editor,
    block: crate::logic_game::app::GameBlock,
    game: Game,
    challenge: ChallengeId,
    level: Memo<Option<Level>>,
    expanded: Memo<bool>,
    on_toggle: ClickCallback,
) -> NodeId {
    let completed = create_memo(move || level.get().is_some_and(|level| level.completed));
    let attempts = create_memo(move || {
        let count = level.get().map_or(0, |level| level.solutions.len());
        format!("{count} attempts")
    });
    let quiz = challenge == ChallengeId::BinaryAddition;
    let shows_quiz = create_memo(move || expanded.get() && quiz);
    let shows_solutions = create_memo(move || expanded.get() && !quiz);
    let goal = generate_challenge(challenge).goal;
    view! {
        <List spacing=4.0>
            <ListRow
                selected={expanded}
                @test_id={format!("logic-game.level.{}", challenge as usize)}
                on_click={on_toggle}
                on_activate={move || {}}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Text>{challenge.name()}</Text>
                    <Show condition={completed}>
                        <Text color=TextColor::Success align=TextAlign::Center>
                            <Icon glyph=ICON_CIRCLE_CHECK />
                        </Text>
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Text style=TextStyle::CAPTION>{attempts}</Text>
                </List>
            </ListRow>
            <Show condition={expanded}>|| {
                let goal = goal.clone(); view! {
                    <Frame padding={Sides::new(INDENT, 0.0, 0.0, 0.0)}>
                        <List spacing=ROW_SPACING>
                            <Text style=TextStyle::CAPTION>{goal}</Text>
                            <Show condition={shows_quiz}>
                                <BinaryAddition editor={editor} block={block} />
                            </Show>
                            <Show condition={shows_solutions}>
                                <Solutions
                                    editor={editor}
                                    game={game}
                                    challenge={challenge}
                                    level={level}
                                />
                            </Show>
                        </List>
                    </Frame>
                }
            }</Show>
        </List>
    }
}

#[component]
fn Solutions(
    editor: Editor,
    game: Game,
    challenge: ChallengeId,
    level: Memo<Option<Level>>,
) -> NodeId {
    let read_only = editor.read_only();
    let keys = create_memo(move || {
        level.get().map_or_else(Vec::new, |level| {
            level
                .solutions
                .iter()
                .map(|solution| solution.reference)
                .collect::<Vec<Uuid>>()
        })
    });
    let start = move || {
        let index = level
            .get_untracked()
            .map_or(0, |level| level.solutions.len());
        game.start(challenge, index);
    };
    view! {
        <List spacing=4.0>
            <ForEach keys={keys}>|reference: Uuid| {
                let solution = solution_of(level, reference);
                view! {
                    <SolutionRow
                        game={game}
                        challenge={challenge}
                        reference={reference}
                        solution={solution}
                    />
                }
            }</ForEach>
            <List direction=Direction::Horizontal spacing=0.0>
                <Button
                    glyph=ICON_PLUS
                    label="New attempt"
                    variant=ButtonVariant::Secondary
                    disabled={read_only}
                    @test_id={format!("logic-game.new-attempt.{}", challenge as usize)}
                    on_click={start}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </List>
    }
}

#[component]
fn SolutionRow(
    game: Game,
    challenge: ChallengeId,
    reference: Uuid,
    solution: Memo<Option<Solution>>,
) -> NodeId {
    let name = create_memo(move || {
        solution
            .get()
            .map(|solution| solution.name)
            .unwrap_or_default()
    });
    let missing = create_memo(move || solution.get().is_none_or(|solution| solution.id.is_none()));
    let passed = create_memo(move || solution.get().is_some_and(|solution| solution.completed));
    let open = move || {
        if let Some(id) = solution.get_untracked().and_then(|solution| solution.id) {
            game.open_solution(id);
        }
    };
    let remove = move || game.remove(challenge, reference);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Link label={name} disabled={missing} on_click={open} />
            <Show condition={passed}>
                <Text color=TextColor::Success align=TextAlign::Center>
                    <Icon glyph=ICON_CIRCLE_CHECK />
                </Text>
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Button
                icon_only=true
                glyph=ICON_TRASH
                label="Remove from this level"
                on_click={remove}
            />
        </List>
    }
}

fn level_of(levels: Memo<Vec<Level>>, challenge: ChallengeId) -> Memo<Option<Level>> {
    create_memo(move || {
        levels.with(|levels| {
            levels
                .iter()
                .find(|level| level.challenge == challenge)
                .cloned()
        })
    })
}

fn solution_of(level: Memo<Option<Level>>, reference: Uuid) -> Memo<Option<Solution>> {
    create_memo(move || {
        level.get().and_then(|level| {
            level
                .solutions
                .into_iter()
                .find(|solution| solution.reference == reference)
        })
    })
}
