use std::rc::Rc;

use block_client::blocks::checklist::{Checklist, ChecklistItem};
use block_editor_plugin::beui::reactive::{
    CenteredRow, Column, ForEach, Frame, ItemSize, KeyedStore, Scroll, Show, WriteSignal, build,
    clone, create_memo, create_selector, create_signal, view, with_reactive_scope,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Checkbox, Heading, Progress, TextInput,
    ToggleButton, use_theme,
};
use block_editor_plugin::beui::{Color32, Context, Document, NodeId, Rect, TextAlign};
use uuid::Uuid;

use super::ChecklistEditor;

const PAGE_PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 18.0;

type Items = KeyedStore<Uuid, ChecklistItem>;

pub struct ChecklistUi {
    document: Document,
    checklist: Rc<ChecklistEditor>,
}

impl ChecklistUi {
    pub(super) fn new(checklist: Rc<ChecklistEditor>) -> Self {
        let root = checklist.clone();
        let document = build(move || {
            view! {
                <ChecklistView checklist={root} />
            }
        });
        Self {
            document,
            checklist,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn background(&self) -> Color32 {
        self.document.theme().background
    }

    pub(super) fn pump(&mut self) {
        let source = self.checklist.source().clone();
        with_reactive_scope(&mut self.document, move || source.pump());
    }

    pub(super) fn show(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }
}

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

#[block_editor_plugin::beui::reactive::component]
fn ChecklistView(checklist: Rc<ChecklistEditor>) -> NodeId {
    let source = checklist.source();
    let items: Items = source.project_keyed(|checklist, items| {
        items.reconcile(checklist.items().iter().map(|item| (item.id, item)));
    });
    let flags = source.project(|checklist| {
        checklist
            .items()
            .iter()
            .map(|item| (item.id, item.done))
            .collect::<Vec<_>>()
    });
    let done_count = source.project(Checklist::done_count);
    let total = source.project(|checklist| checklist.items().len());

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
                .collect::<Vec<Uuid>>()
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
    let rows = clone!(checklist items -> move |id: Uuid| {
        view! {
            <ChecklistRow checklist={checklist.clone()} items={items.clone()} id />
        }
    });

    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=PAGE_PADDING
            padding_vertical=PAGE_PADDING
        >
            <Column spacing=SECTION_SPACING>
                <Column spacing=6.0>
                    <Heading content="Checklist" />
                    <Caption content={summary} />
                    <Progress value={progress} label="Checklist completion" />
                </Column>
                <Card>
                    <Column spacing=12.0>
                        <CenteredRow spacing=10.0>
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
                        </CenteredRow>
                        <CenteredRow spacing=10.0>
                            <CenteredRow @sizing=ItemSize::Percent(100.0) spacing=6.0>
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
                            </CenteredRow>
                            <Button
                                label="Clear completed"
                                variant=ButtonVariant::Secondary
                                disabled={clear_disabled}
                                @test_id={"checklist.clear-done"}
                                on_click={move || clear_checklist.clear_done()}
                            />
                        </CenteredRow>
                    </Column>
                </Card>
                <Card @sizing=ItemSize::Percent(100.0)>
                    <Column spacing=10.0>
                        <Show condition={empty}>
                            <Body content="No tasks match this view." align=TextAlign::Center />
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0) focus_color={theme.accent.clone()}>
                            <ForEach spacing=8.0 keys={visible} view={rows} />
                        </Scroll>
                    </Column>
                </Card>
            </Column>
        </Frame>
    }
}

fn add_item(checklist: &Rc<ChecklistEditor>, set_draft: &WriteSignal<String>, value: String) {
    let text = value.trim().to_owned();
    if text.is_empty() {
        return;
    }
    checklist.add(text);
    set_draft.set(String::new());
}

#[block_editor_plugin::beui::reactive::component]
fn ChecklistRow(checklist: Rc<ChecklistEditor>, items: Items, id: Uuid) -> NodeId {
    let item = items.get(&id);
    let label = create_memo(clone!(item -> move || item.with(|item| item.text.clone())));
    let done = create_memo(clone!(item -> move || item.with(|item| item.done)));
    let toggle = checklist.clone();
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            radius=6
            padding_horizontal=12.0
            padding_vertical=10.0
        >
            <CenteredRow spacing=10.0>
                <Checkbox
                    @sizing=ItemSize::Percent(100.0)
                    label={label}
                    checked={done}
                    @test_id={format!("checklist.item.{id}.done")}
                    on_change={move |done| toggle.set_done(id, done)}
                />
                <Button
                    label="Remove"
                    variant=ButtonVariant::Secondary
                    @test_id={format!("checklist.item.{id}.remove")}
                    on_click={move || checklist.remove(id)}
                />
            </CenteredRow>
        </Frame>
    }
}
