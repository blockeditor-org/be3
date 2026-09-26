use std::rc::Rc;

use block_editor_beui::be_block::{
    Checklist as ChecklistModel, ChecklistContent, ChecklistItem, ObjectId,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Show, WriteSignal, clone, component,
    create_memo, create_selector, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Checkbox, Heading, Progress, Scroll, TextInput,
    ToggleButton, use_theme,
};
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{ContentProjection, Editor};

const PAGE_PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 18.0;

type List = Rc<ContentProjection<ChecklistContent>>;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum Filter {
    All,
    Open,
    Done,
}

impl Filter {
    fn keeps(self, done: bool) -> bool {
        match self {
            Self::All => true,
            Self::Open => !done,
            Self::Done => done,
        }
    }
}

#[component]
pub fn Checklist(editor: Editor) -> NodeId {
    let checklist: List = editor.block_content::<ChecklistContent>();
    let flags = checklist.project(|checklist| {
        checklist
            .root()
            .items
            .iter()
            .map(|item| (item.id, item.done))
            .collect::<Vec<_>>()
    });
    let done_count = checklist.project(|checklist| checklist.root().done_count());
    let ids = checklist.ids(ObjectId::ROOT, ChecklistModel::ITEMS);
    let total = create_memo(clone!(ids -> move || ids.with(Vec::len)));

    let (draft, set_draft) = create_signal(String::new());
    let (filter, set_filter) = create_signal(Filter::All);
    let selected_filter = create_selector(clone!(filter -> move || filter.get()));
    let visible = create_memo(clone!(flags filter -> move || {
        let filter = filter.get();
        flags.with(|flags| {
            flags
                .iter()
                .filter(|(_, done)| filter.keeps(*done))
                .map(|(id, _)| *id)
                .collect::<Vec<ObjectId>>()
        })
    }));
    let progress = create_memo(clone!(done_count total -> move || {
        match total.get() {
            0 => 0.0,
            total => done_count.get() as f32 / total as f32,
        }
    }));
    let summary = create_memo(clone!(done_count total -> move || {
        match total.get() {
            0 => "Add your first task below".to_owned(),
            total => format!("{} of {total} complete", done_count.get()),
        }
    }));
    let empty = create_memo(clone!(visible -> move || visible.get().is_empty()));
    let clear_disabled = create_memo(clone!(done_count -> move || done_count.get() == 0));

    let submit_checklist = checklist.clone();
    let submit_draft = set_draft.clone();
    let add_checklist = checklist.clone();
    let add_draft = draft.clone();
    let add_set_draft = set_draft.clone();
    let clear_checklist = checklist.clone();
    let set_open_filter = set_filter.clone();
    let set_done_filter = set_filter.clone();
    let rows = clone!(checklist -> move |id: ObjectId| {
        view! {
            <ChecklistRow checklist={checklist.clone()} id />
        }
    });

    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=PAGE_PADDING
            padding_vertical=PAGE_PADDING
        >
            <List spacing=SECTION_SPACING>
                <List spacing=6.0>
                    <Heading content="Checklist" />
                    <Caption content={summary} />
                    <Progress value={progress} label="Checklist completion" />
                </List>
                <Card>
                    <List spacing=12.0>
                        <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                            <TextInput
                                @sizing=ItemSize::Percent(100.0)
                                value={draft}
                                placeholder="What needs doing?"
                                label="New checklist item"
                                @test_id={"checklist.draft"}
                                on_change={move |value| set_draft.set(value)}
                                on_submit={move |value| {
                                    add_item(&submit_checklist, &submit_draft, value);
                                }}
                            />
                            <Button
                                label="Add task"
                                variant=ButtonVariant::Primary
                                @test_id={"checklist.add"}
                                on_click={move || {
                                    add_item(&add_checklist, &add_set_draft, add_draft.get());
                                }}
                            />
                        </List>
                        <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                            <List
                                @sizing=ItemSize::Percent(100.0)
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=6.0
                            >
                                <ToggleButton
                                    label="All"
                                    pressed={selected_filter.memo(Filter::All)}
                                    @test_id={"checklist.filter.all"}
                                    on_change={move |_| set_filter.set(Filter::All)}
                                />
                                <ToggleButton
                                    label="Open"
                                    pressed={selected_filter.memo(Filter::Open)}
                                    @test_id={"checklist.filter.open"}
                                    on_change={move |_| set_open_filter.set(Filter::Open)}
                                />
                                <ToggleButton
                                    label="Done"
                                    pressed={selected_filter.memo(Filter::Done)}
                                    @test_id={"checklist.filter.done"}
                                    on_change={move |_| set_done_filter.set(Filter::Done)}
                                />
                            </List>
                            <Button
                                label="Clear completed"
                                variant=ButtonVariant::Secondary
                                disabled={clear_disabled}
                                @test_id={"checklist.clear-done"}
                                on_click={move || {
                                    if let Some(edit) = clear_checklist.read(|content| content.root().clear_done()) {
                                        clear_checklist.operate(edit);
                                    }
                                }}
                            />
                        </List>
                    </List>
                </Card>
                <Card @sizing=ItemSize::Percent(100.0)>
                    <List spacing=10.0>
                        <Show condition={empty}>
                            <Body content="No tasks match this view." align=TextAlign::Center />
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0)>
                            <List spacing=8.0>
                                <ForEach keys={visible} view={rows} />
                            </List>
                        </Scroll>
                    </List>
                </Card>
            </List>
        </Frame>
    }
}

fn add_item(checklist: &List, set_draft: &WriteSignal<String>, value: String) {
    let text = value.trim().to_owned();
    if text.is_empty() {
        return;
    }
    checklist.operate(ChecklistModel::add(text).1);
    set_draft.set(String::new());
}

#[component]
fn ChecklistRow(checklist: List, id: ObjectId) -> NodeId {
    let item = checklist.object::<ChecklistItem>(id);
    let label = create_memo(clone!(item -> move || {
        item.with(|item| item.as_ref().map(|item| item.text.clone()).unwrap_or_default())
    }));
    let done = create_memo(clone!(item -> move || {
        item.with(|item| item.as_ref().is_some_and(|item| item.done))
    }));
    let toggle = checklist.clone();
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            radius=6
            padding_horizontal=12.0
            padding_vertical=10.0
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <Checkbox
                    @sizing=ItemSize::Percent(100.0)
                    label={label}
                    checked={done}
                    @test_id={format!("checklist.item.{id}.done")}
                    on_change={move |done| {
                        toggle.operate(ChecklistModel::set_done(id, done));
                    }}
                />
                <Button
                    label="Remove"
                    variant=ButtonVariant::Secondary
                    @test_id={format!("checklist.item.{id}.remove")}
                    on_click={move || checklist.operate(ChecklistModel::remove(id))}
                />
            </List>
        </Frame>
    }
}
