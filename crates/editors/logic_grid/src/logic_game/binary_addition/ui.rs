use std::rc::Rc;

use block_editor_beui::Editor;
use block_editor_beui::be_block::logic_game::QuizRow;
use block_editor_beui::beui::icons::ICON_CHECK_CIRCLE;
use block_editor_beui::beui::reactive::{
    Align, Callback, ClickCallback, Direction, ForEach, Frame, ItemSize, List, Memo, ReadSignal,
    Show, Spacer, clone, component, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Bordered, Button, ButtonVariant, Caption, Code, Icon, use_theme,
};
use block_editor_beui::beui::{NodeId, TextAlign};

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
    let quiz = Rc::new(BinaryAdditionQuiz::default());
    let count = quiz.problems().len();
    let (page, set_page) = create_signal(0usize);
    let (checked, set_checked) = create_signal(false);
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
    let answers = create_memo(clone!(quiz stored page -> move || {
        let problem = page.get();
        let (carries, sums) = stored.with(|stored| stored[problem].clone());
        quiz.fit(problem, carries, sums)
    }));
    let correct = create_memo(clone!(quiz answers page -> move || {
        answers.with(|(carries, sums)| quiz.is_correct(carries, sums, page.get()))
    }));
    let finished = create_memo(clone!(quiz stored -> move || {
        stored.with(|stored| {
            stored.iter().enumerate().all(|(problem, (carries, sums))| {
                let (carries, sums) = quiz.fit(problem, carries.clone(), sums.clone());
                quiz.is_correct(&carries, &sums, problem)
            })
        })
    }));

    let read_only = editor.read_only();
    let grid_read_only = read_only.clone();
    let title = create_memo(clone!(page -> move || {
        format!("Problem {} of {count}", page.get() + 1)
    }));
    let first = create_memo(clone!(page -> move || page.get() == 0));
    let last = create_memo(clone!(page -> move || page.get() + 1 >= count));
    let not_last = create_memo(clone!(last -> move || !last.get()));
    let blocked = create_memo(clone!(correct -> move || !correct.get()));
    let passed = create_memo(clone!(checked correct -> move || checked.get() && correct.get()));
    let failed = create_memo(clone!(checked correct -> move || checked.get() && !correct.get()));
    let done = create_memo(clone!(last correct finished -> move || {
        last.get() && correct.get() && finished.get()
    }));

    let check = clone!(set_checked -> move || set_checked.set(true));
    let reset = clone!(quiz block page set_checked -> move || {
        let problem = page.get_untracked();
        quiz.write_row(&block, problem, QuizRow::Carries, Vec::new());
        quiz.write_row(&block, problem, QuizRow::Sums, Vec::new());
        set_checked.set(false);
    });
    let back = clone!(page set_page set_checked -> move || {
        set_page.set(page.get_untracked().saturating_sub(1));
        set_checked.set(false);
    });
    let forward = clone!(page set_page set_checked -> move || {
        set_page.set((page.get_untracked() + 1).min(count - 1));
        set_checked.set(false);
    });

    let theme = use_theme();
    let success = theme.success.clone();
    view! {
        <List spacing=ROW_SPACING>
            <Body content={title} />
            <Bordered corner_radius=8>
                <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                    <ProblemGrid
                        quiz={Rc::clone(&quiz)}
                        block={block}
                        page={page.clone()}
                        answers={answers}
                        checked={checked.clone()}
                        read_only={grid_read_only}
                    />
                </Frame>
            </Bordered>
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
                    <Caption content="This page is correct." color={theme.success.clone()} />
                </Show>
                <Show condition={failed}>
                    <Caption content="Some blanks still need work." color={theme.danger.clone()} />
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
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Icon glyph={ICON_CHECK_CIRCLE.to_owned()} color={success.clone()} />
                        <Caption content="Every problem is correct." color={success} />
                    </List>
                </Show>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </List>
    }
}

#[component]
fn ProblemGrid(
    quiz: Rc<BinaryAdditionQuiz>,
    block: crate::logic_game::app::GameBlock,
    page: ReadSignal<usize>,
    answers: Memo<Answers>,
    checked: ReadSignal<bool>,
    read_only: Memo<bool>,
) -> NodeId {
    let operands = create_memo(clone!(quiz page -> move || {
        quiz.problems()[page.get()].operands().to_vec()
    }));
    let write = clone!(quiz block page -> move |row: QuizRow, values: Vec<Option<bool>>| {
        quiz.write_row(&block, page.get_untracked(), row, values);
    });
    let carry_write = write.clone();
    let carries =
        create_memo(clone!(answers -> move || answers.with(|(carries, _)| carries.clone())));
    let sums = create_memo(clone!(answers -> move || answers.with(|(_, sums)| sums.clone())));
    let carry_expected = create_memo(clone!(quiz page -> move || {
        quiz.problems()[page.get()].carry_bits().to_vec()
    }));
    let sum_expected = create_memo(clone!(quiz page -> move || {
        quiz.problems()[page.get()].sum_bits().to_vec()
    }));
    let theme = use_theme();
    view! {
        <List spacing=ROW_SPACING>
            <BitRow
                label="carry"
                answers={carries}
                expected={carry_expected}
                checked={checked.clone()}
                read_only={read_only.clone()}
                prefix="quiz.carry"
                on_change={move |values| carry_write(QuizRow::Carries, values)}
            />
            <OperandRows operands={operands} />
            <List direction=Direction::Horizontal align=Align::Center spacing=CELL_SPACING>
                <Spacer @sizing=ItemSize::Fixed(LABEL_WIDTH) />
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    height=RULE_HEIGHT
                    color={theme.border.clone()}
                    radius=0
                />
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
    let rows = create_memo(
        clone!(operands -> move || (0..operands.with(Vec::len)).collect::<Vec<usize>>()),
    );
    view! {
        <List spacing=ROW_SPACING>
            <ForEach keys={rows}>
                {move |index: usize| {
                    let operand = create_memo(clone!(operands -> move || {
                        operands.with(|operands| operands.get(index).cloned().unwrap_or_default())
                    }));
                    let sign = create_memo(clone!(operands -> move || {
                        match index + 1 == operands.with(Vec::len) {
                            true => "+".to_owned(),
                            false => String::new(),
                        }
                    }));
                    view! {
                        <OperandRow sign={sign} operand={operand} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn OperandRow(sign: Memo<String>, operand: Memo<String>) -> NodeId {
    let bits = create_memo(
        clone!(operand -> move || (0..operand.with(String::len)).collect::<Vec<usize>>()),
    );
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CELL_SPACING>
            <Frame width=LABEL_WIDTH>
                <Caption content={sign} align=TextAlign::End />
            </Frame>
            <ForEach keys={bits}>
                {move |column: usize| {
                    let bit = create_memo(clone!(operand -> move || {
                        operand.with(|operand| {
                            operand.chars().nth(column).map(String::from).unwrap_or_default()
                        })
                    }));
                    view! {
                        <Frame width=CELL>
                            <Code content={bit} align=TextAlign::Center />
                        </Frame>
                    }
                }}
            </ForEach>
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
    let columns =
        create_memo(clone!(answers -> move || (0..answers.with(Vec::len)).collect::<Vec<usize>>()));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CELL_SPACING>
            <Frame width=LABEL_WIDTH>
                <Caption content={label} align=TextAlign::End />
            </Frame>
            <ForEach keys={columns}>
                {move |column: usize| {
                    let answer = create_memo(clone!(answers -> move || {
                        answers.with(|answers| answers.get(column).copied().flatten())
                    }));
                    let wrong = create_memo(clone!(answers expected checked -> move || {
                        checked.get()
                            && answers.with(|answers| answers.get(column).copied().flatten())
                                != expected.with(|expected| expected.get(column).copied())
                    }));
                    let on_change = on_change.clone();
                    let answers = answers.clone();
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
                            read_only={read_only.clone()}
                            @test_id={format!("{prefix}.{column}")}
                            on_click={cycle}
                        />
                    }
                }}
            </ForEach>
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
    let outline = create_memo(clone!(theme -> move || theme.danger.get()));
    view! {
        <Frame
            width=CELL
            outline={outline}
            outline_width=1.0
            outline_offset=1.0
            outline_visible={wrong}
            radius=4
        >
            <Button
                label={label}
                variant=ButtonVariant::Secondary
                disabled={read_only}
                on_click={move || on_click.call()}
            />
        </Frame>
    }
}
