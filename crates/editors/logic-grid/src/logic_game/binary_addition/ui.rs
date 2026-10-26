use block_editor_beui::Editor;
use block_editor_beui::beui::icons::ICON_CIRCLE_CHECK;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use logic_grid_types::QuizRow;

use super::{BinaryAdditionQuiz, next_answer};

type Answers = (Vec<Option<bool>>, Vec<Option<bool>>);

const CELL: f32 = 24.0;
const CELL_SPACING: f32 = 4.0;
const ROW_SPACING: f32 = 6.0;
const LABEL_WIDTH: f32 = 46.0;
const RULE_HEIGHT: f32 = 1.0;
const PADDING: f32 = 8.0;

#[component]
pub(crate) fn BinaryAddition(editor: Editor, block: crate::logic_game::app::GameBlock) -> NodeId {
    let quiz = store(BinaryAdditionQuiz::default());
    let count = quiz.value().problems().len();
    let page = create_signal(0usize);
    let checked = create_signal(false);
    let stored = block.project(move |game| {
        (0..count)
            .map(|problem| {
                game.root()
                    .game()
                    .quiz(problem)
                    .map(|answers| (answers.carries.clone(), answers.sums.clone()))
                    .unwrap_or_default()
            })
            .collect::<Vec<Answers>>()
    });
    let answers = create_memo(move || {
        let problem = page.get();
        let (carries, sums) = stored.with(|stored| stored[problem].clone());
        quiz.value().fit(problem, carries, sums)
    });
    let correct = create_memo(move || {
        answers.with(|(carries, sums)| quiz.value().is_correct(carries, sums, page.get()))
    });
    let finished = create_memo(move || {
        stored.with(|stored| {
            stored.iter().enumerate().all(|(problem, (carries, sums))| {
                let (carries, sums) = quiz.value().fit(problem, carries.clone(), sums.clone());
                quiz.value().is_correct(&carries, &sums, problem)
            })
        })
    });

    let read_only = editor.read_only();
    let title = create_memo(move || format!("Problem {} of {count}", page.get() + 1));
    let first = create_memo(move || page.get() == 0);
    let last = create_memo(move || page.get() + 1 >= count);
    let not_last = create_memo(move || !last.get());
    let blocked = create_memo(move || !correct.get());
    let passed = create_memo(move || checked.get() && correct.get());
    let failed = create_memo(move || checked.get() && !correct.get());
    let done = create_memo(move || last.get() && correct.get() && finished.get());

    let check = move || checked.set(true);
    let reset = move || {
        let problem = page.get_untracked();
        quiz.value()
            .write_row(&block, problem, QuizRow::Carries, Vec::new());
        quiz.value()
            .write_row(&block, problem, QuizRow::Sums, Vec::new());
        checked.set(false);
    };
    let back = move || {
        page.set(page.get_untracked().saturating_sub(1));
        checked.set(false);
    };
    let forward = move || {
        page.set((page.get_untracked() + 1).min(count - 1));
        checked.set(false);
    };

    view! {
        <List spacing=ROW_SPACING>
            <Text>{title}</Text>
            <Frame border={Border::DEFAULT} radius={Radius::custom(8.0)} padding=PADDING>
                <ProblemGrid
                    quiz={quiz}
                    block={block}
                    page={page.read_only()}
                    answers={answers}
                    checked={checked.read_only()}
                    read_only={read_only}
                />
            </Frame>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Check"
                    variant=ButtonVariant::Secondary
                    @test_id={"quiz.check"}
                    on_click={check}
                />
                <Button
                    label="Reset page"
                    variant=ButtonVariant::Secondary
                    disabled={read_only}
                    @test_id={"quiz.reset"}
                    on_click={reset}
                />
                <Show condition={passed}>
                    <Text style=TextStyle::CAPTION color=TextColor::Success>
                        "This page is correct."
                    </Text>
                </Show>
                <Show condition={failed}>
                    <Text style=TextStyle::CAPTION color=TextColor::Danger>
                        "Some blanks still need work."
                    </Text>
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Previous"
                    variant=ButtonVariant::Secondary
                    disabled={first}
                    @test_id={"quiz.previous"}
                    on_click={back}
                />
                <Show condition={not_last}>
                    <Button
                        label="Next"
                        variant=ButtonVariant::Primary
                        disabled={blocked}
                        @test_id={"quiz.next"}
                        on_click={forward}
                    />
                </Show>
                <Show condition={done}>
                    <Text style=TextStyle::CAPTION color=TextColor::Success>
                        <Icon glyph=ICON_CIRCLE_CHECK />
                        "Every problem is correct."
                    </Text>
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </List>
    }
}

#[component]
fn ProblemGrid(
    quiz: Stored<BinaryAdditionQuiz>,
    block: crate::logic_game::app::GameBlock,
    page: ReadSignal<usize>,
    answers: Memo<Answers>,
    checked: ReadSignal<bool>,
    read_only: Memo<bool>,
) -> NodeId {
    let operands = create_memo(move || quiz.value().problems()[page.get()].operands().to_vec());
    let write = move |row: QuizRow, values: Vec<Option<bool>>| {
        quiz.value()
            .write_row(&block, page.get_untracked(), row, values);
    };
    let carries = create_memo(move || answers.with(|(carries, _)| carries.clone()));
    let sums = create_memo(move || answers.with(|(_, sums)| sums.clone()));
    let carry_expected =
        create_memo(move || quiz.value().problems()[page.get()].carry_bits().to_vec());
    let sum_expected = create_memo(move || quiz.value().problems()[page.get()].sum_bits().to_vec());
    view! {
        <List spacing=ROW_SPACING>
            <BitRow
                label="carry"
                answers={carries}
                expected={carry_expected}
                checked={checked}
                read_only={read_only}
                prefix="quiz.carry"
                on_change={move |values| write(QuizRow::Carries, values)}
            />
            <OperandRows operands={operands} />
            <List direction=Direction::Horizontal align=Align::Center spacing=CELL_SPACING>
                <Spacer @sizing=ItemSize::Fixed(LABEL_WIDTH) />
                <Frame @sizing=ItemSize::Percent(100.0) height=RULE_HEIGHT fill=ColorRole::Border />
            </List>
            <BitRow
                label="sum"
                answers={sums}
                expected={sum_expected}
                checked={checked}
                read_only={read_only}
                prefix="quiz.sum"
                on_change={move |values| write(QuizRow::Sums, values)}
            />
        </List>
    }
}

#[component]
fn OperandRows(operands: Memo<Vec<String>>) -> NodeId {
    let rows = create_memo(move || (0..operands.with(Vec::len)).collect::<Vec<usize>>());
    view! {
        <List spacing=ROW_SPACING>
            <ForEach keys={rows}>|index: usize| {
                let operand = create_memo(move || {
                    operands.with(|operands| operands.get(index).cloned().unwrap_or_default())
                });
                let sign = create_memo(move || {
                    match index + 1 == operands.with(Vec::len) {
                        true => "+".to_owned(),
                        false => String::new(),
                    }
                });
                view! {
                    <OperandRow sign={sign} operand={operand} />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn OperandRow(sign: Memo<String>, operand: Memo<String>) -> NodeId {
    let bits = create_memo(move || (0..operand.with(String::len)).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CELL_SPACING>
            <Frame width=LABEL_WIDTH>
                <Text style=TextStyle::CAPTION align=TextAlign::End>{sign}</Text>
            </Frame>
            <ForEach keys={bits}>|column: usize| {
                let bit = create_memo(move || {
                    operand.with(|operand| {
                        operand.chars().nth(column).map(String::from).unwrap_or_default()
                    })
                });
                view! {
                    <Frame width=CELL>
                        <Text style=TextStyle::CODE align=TextAlign::Center>{bit}</Text>
                    </Frame>
                }
            }</ForEach>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn BitRow(
    label: &'static str,
    answers: Memo<Vec<Option<bool>>>,
    expected: Memo<Vec<bool>>,
    checked: ReadSignal<bool>,
    read_only: Memo<bool>,
    prefix: &'static str,
    on_change: Callback<Vec<Option<bool>>>,
) -> NodeId {
    let columns = create_memo(move || (0..answers.with(Vec::len)).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CELL_SPACING>
            <Frame width=LABEL_WIDTH>
                <Text style=TextStyle::CAPTION align=TextAlign::End>{label}</Text>
            </Frame>
            <ForEach keys={columns}>|column: usize| {
                let answer = create_memo(move || {
                    answers.with(|answers| answers.get(column).copied().flatten())
                });
                let wrong = create_memo(move || {
                    checked.get()
                        && answers.with(|answers| answers.get(column).copied().flatten())
                            != expected.with(|expected| expected.get(column).copied())
                });
                let cycle = move || {
                    let mut values = answers.get_untracked();
                    if let Some(value) = values.get_mut(column) {
                        *value = next_answer(*value);
                    }
                    on_change.call(values);
                };
                view! {
                    <BitCell
                        answer={answer}
                        wrong={wrong}
                        read_only={read_only}
                        @test_id={format!("{prefix}.{column}")}
                        on_click={cycle}
                    />
                }
            }</ForEach>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn BitCell(
    answer: Memo<Option<bool>>,
    wrong: Memo<bool>,
    read_only: Memo<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let label = create_memo(move || match answer.get() {
        Some(bit) => u8::from(bit).to_string(),
        None => " ".to_owned(),
    });
    let theme = use_theme();
    let outline = create_memo(move || theme.danger.get());
    view! {
        <Frame
            width=CELL
            border={create_memo(move || wrong.get().then(|| Border::new(ColorRole::Custom(outline.get())).width(1.0).offset(1.0)))}
            radius=Radius::Sm
        >
            <Button
                label={label}
                variant=ButtonVariant::Secondary
                disabled={read_only}
                on_click={on_click}
            />
        </Frame>
    }
}
