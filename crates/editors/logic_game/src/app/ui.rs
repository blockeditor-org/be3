use std::rc::Rc;
use uuid::Uuid;

use block_editor_plugin::Editor;
use block_editor_plugin::beui::icons::{ICON_ADD, ICON_CHECK_CIRCLE, ICON_DELETE, ICON_WIDGETS};
use block_editor_plugin::beui::reactive::ClickCallback;
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, NodeRef, Show, Spacer, clone,
    component, create_effect, create_memo, create_selector, create_signal, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, Icon, IconButton, Link, ListRow, Scroll,
    use_theme,
};
use block_editor_plugin::beui::{NodeId, Vec2};
use logicgame::challenges::{ChallengeId, generate_challenge};

use crate::binary_addition::ui::BinaryAddition;

use super::game::{Game, Level, Solution};

const PADDING: f32 = 16.0;
const ROW_SPACING: f32 = 8.0;
const INDENT: f32 = 20.0;
const ROW_HEIGHT: f32 = 26.0;
const CHROME_HEIGHT: f32 = 120.0;
const QUIZ_HEIGHT: f32 = 320.0;
const INTRINSIC_WIDTH: f32 = 720.0;

#[component]
pub fn LogicGameEditor(editor: Editor) -> NodeId {
    let block = editor.block_content::<block_editor_plugin::be_block::LogicGameContent>();
    let game = Rc::new(Game::watch(&editor, Rc::clone(&block)));
    let levels = game.levels();
    let hotbar = game.hotbar();
    let (expanded, set_expanded) = create_signal(None::<ChallengeId>);

    let sized = editor.clone();
    create_effect(clone!(levels expanded -> move || {
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
        sized.set_intrinsic_size(Some(Vec2::new(
            INTRINSIC_WIDTH,
            CHROME_HEIGHT + ROW_HEIGHT * rows as f32 + quiz,
        )));
    }));

    let has_hotbar = create_memo(clone!(hotbar -> move || hotbar.get().is_some()));
    let open_hotbar = clone!(game -> move || game.open_hotbar());
    let keys = create_memo(clone!(levels -> move || {
        levels.with(|levels| levels.iter().map(|level| level.challenge).collect::<Vec<_>>())
    }));
    let open = create_selector(clone!(expanded -> move || expanded.get()));

    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <Frame @node_ref={&content} padding_horizontal=PADDING padding_vertical=PADDING>
                <List spacing=ROW_SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                        <Heading content="Levels" />
                        <Show condition={has_hotbar}>
                            <IconButton
                                glyph={ICON_WIDGETS.to_owned()}
                                label="Hotbar"
                                @test_id={"logic-game.hotbar"}
                                on_click={open_hotbar}
                            />
                        </Show>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                    </List>
                    <Scroll @sizing=ItemSize::Percent(100.0)>
                        <List spacing=4.0>
                            <ForEach keys={keys}>
                                {move |challenge: ChallengeId| {
                                    let level = level_of(levels.clone(), challenge);
                                    let here = open.memo(Some(challenge));
                                    let toggle = clone!(expanded set_expanded -> move || {
                                        let shown = expanded.get_untracked() == Some(challenge);
                                        set_expanded.set((!shown).then_some(challenge));
                                    });
                                    view! {
                                        <LevelRow
                                            editor={editor.clone()}
                                            block={Rc::clone(&block)}
                                            game={Rc::clone(&game)}
                                            challenge={challenge}
                                            level={level}
                                            expanded={here}
                                            on_toggle={toggle}
                                        />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Scroll>
                </List>
            </Frame>
        </Frame>
    }
}

#[component]
fn LevelRow(
    editor: Editor,
    block: crate::app::GameBlock,
    game: Rc<Game>,
    challenge: ChallengeId,
    level: Memo<Option<Level>>,
    expanded: Memo<bool>,
    on_toggle: ClickCallback,
) -> NodeId {
    let completed = create_memo(clone!(level -> move || {
        level.get().is_some_and(|level| level.completed)
    }));
    let attempts = create_memo(clone!(level -> move || {
        let count = level.get().map_or(0, |level| level.solutions.len());
        format!("{count} attempts")
    }));
    let quiz = challenge == ChallengeId::BinaryAddition;
    let shows_quiz = create_memo(clone!(expanded -> move || expanded.get() && quiz));
    let shows_solutions = create_memo(clone!(expanded -> move || expanded.get() && !quiz));
    let goal = generate_challenge(challenge).goal;
    let solving = editor.clone();
    let theme = use_theme();
    view! {
        <List spacing=4.0>
            <ListRow
                selected={expanded.clone()}
                @test_id={format!("logic-game.level.{}", challenge as usize)}
                on_click={move || on_toggle.call()}
                on_activate={move || {}}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Body content={challenge.name()} />
                    <Show condition={completed}>
                        <Icon glyph={ICON_CHECK_CIRCLE.to_owned()} color={theme.success.clone()} />
                    </Show>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Caption content={attempts} />
                </List>
            </ListRow>
            <Show condition={expanded}>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Spacer @sizing=ItemSize::Fixed(INDENT) />
                    <List @sizing=ItemSize::Percent(100.0) spacing=ROW_SPACING>
                        <Caption content={goal} wrap=true />
                        <Show condition={shows_quiz}>
                            <BinaryAddition editor={editor} block={block} />
                        </Show>
                        <Show condition={shows_solutions}>
                            <Solutions
                                editor={solving}
                                game={game}
                                challenge={challenge}
                                level={level}
                            />
                        </Show>
                    </List>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn Solutions(
    editor: Editor,
    game: Rc<Game>,
    challenge: ChallengeId,
    level: Memo<Option<Level>>,
) -> NodeId {
    let read_only = editor.read_only();
    let keys = create_memo(clone!(level -> move || {
        level.get().map_or_else(Vec::new, |level| {
            level.solutions.iter().map(|solution| solution.reference).collect::<Vec<Uuid>>()
        })
    }));
    let start = clone!(game level -> move || {
        let index = level.get_untracked().map_or(0, |level| level.solutions.len());
        game.start(challenge, index);
    });
    view! {
        <List spacing=4.0>
            <ForEach keys={keys}>
                {move |reference: Uuid| {
                    let solution = solution_of(level.clone(), reference);
                    view! {
                        <SolutionRow
                            game={Rc::clone(&game)}
                            challenge={challenge}
                            reference={reference}
                            solution={solution}
                        />
                    }
                }}
            </ForEach>
            <List direction=Direction::Horizontal spacing=0.0>
                <Button
                    glyph={ICON_ADD.to_owned()}
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
    game: Rc<Game>,
    challenge: ChallengeId,
    reference: Uuid,
    solution: Memo<Option<Solution>>,
) -> NodeId {
    let name = create_memo(clone!(solution -> move || {
        solution.get().map(|solution| solution.name).unwrap_or_default()
    }));
    let missing = create_memo(clone!(solution -> move || {
        solution.get().is_none_or(|solution| solution.id.is_none())
    }));
    let passed = create_memo(clone!(solution -> move || {
        solution.get().is_some_and(|solution| solution.completed)
    }));
    let open = clone!(game solution -> move || {
        if let Some(id) = solution.get_untracked().and_then(|solution| solution.id) {
            game.open_solution(id);
        }
    });
    let remove = clone!(game -> move || game.remove(challenge, reference));
    let theme = use_theme();
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Link label={name} disabled={missing} on_click={open} />
            <Show condition={passed}>
                <Icon glyph={ICON_CHECK_CIRCLE.to_owned()} color={theme.success.clone()} />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <IconButton
                glyph={ICON_DELETE.to_owned()}
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
