use beui::{Align, Direction, ItemSize};
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::{ContentProjection, Editor};
use checklist_types::{Checklist as ChecklistModel, ChecklistContent, ChecklistItem};

const PAGE_PADDING: Space = Space::Xl3;
const SECTION_SPACING: Space = Space::Xl;

type List = ContentProjection<ChecklistContent>;

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
    let total = create_memo(move || ids.with(Vec::len));

    let draft = create_signal(String::new());
    let filter = create_signal(Filter::All);
    let selected_filter = create_selector(move || filter.get());
    let visible = create_memo(move || {
        let filter = filter.get();
        flags.with(|flags| {
            flags
                .iter()
                .filter(|(_, done)| filter.keeps(*done))
                .map(|(id, _)| *id)
                .collect::<Vec<ObjectId>>()
        })
    });
    let progress = create_memo(move || match total.get() {
        0 => Some(0.0),
        total => Some(done_count.get() as f32 / total as f32),
    });
    let summary = create_memo(move || match total.get() {
        0 => "Add your first task below".to_owned(),
        total => format!("{} of {total} complete", done_count.get()),
    });
    let empty = create_memo(move || visible.get().is_empty());
    let clear_disabled = create_memo(move || done_count.get() == 0);

    let rows = move |id: ObjectId| {
        view! {
            <ChecklistRow checklist={checklist} id />
        }
    };

    view! {
        <Frame padding=PAGE_PADDING>
            <List spacing=SECTION_SPACING>
                <List spacing=Space::Sm>
                    <Text style=TextStyle::HEADING>"Checklist"</Text>
                    <Text style=TextStyle::CAPTION>{summary}</Text>
                    <Progress value={progress} label="Checklist completion" />
                </List>
                <Card>
                    <List spacing=Space::Lg>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <TextInput
                                @sizing=ItemSize::Percent(100.0)
                                value={draft}
                                placeholder="What needs doing?"
                                label="New checklist item"
                                @test_id={"checklist.draft"}
                                on_submit={move |value| {
                                    add_item(&checklist, draft, value);
                                }}
                            />
                            <Button
                                label="Add task"
                                variant=ButtonVariant::Primary
                                @test_id={"checklist.add"}
                                on_click={move || {
                                    add_item(&checklist, draft, draft.get());
                                }}
                            />
                        </List>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <List
                                @sizing=ItemSize::Percent(100.0)
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Sm
                            >
                                <ToggleButton
                                    label="All"
                                    pressed={Binding::new(selected_filter.memo(Filter::All), move |_| filter.set(Filter::All))}
                                    @test_id={"checklist.filter.all"}
                                />
                                <ToggleButton
                                    label="Open"
                                    pressed={Binding::new(selected_filter.memo(Filter::Open), move |_| filter.set(Filter::Open))}
                                    @test_id={"checklist.filter.open"}
                                />
                                <ToggleButton
                                    label="Done"
                                    pressed={Binding::new(selected_filter.memo(Filter::Done), move |_| filter.set(Filter::Done))}
                                    @test_id={"checklist.filter.done"}
                                />
                            </List>
                            <Button
                                label="Clear completed"
                                variant=ButtonVariant::Secondary
                                disabled={clear_disabled}
                                @test_id={"checklist.clear-done"}
                                on_click={move || {
                                    if let Ok(edit) = checklist.read(|content| content.root().clear_done()) {
                                        checklist.operate(edit);
                                    }
                                }}
                            />
                        </List>
                    </List>
                </Card>
                <Card @sizing=ItemSize::Percent(100.0)>
                    <List spacing=Space::Md>
                        <Show condition={empty}>
                            <Text align=TextAlign::Center>"No tasks match this view."</Text>
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0)>
                            <List spacing=Space::Md>
                                <ForEach keys={visible} view={rows} />
                            </List>
                        </Scroll>
                    </List>
                </Card>
            </List>
        </Frame>
    }
}

fn add_item(checklist: &List, draft: Signal<String>, value: String) {
    let text = value.trim().to_owned();
    if text.is_empty() {
        return;
    }
    checklist.operate(ChecklistModel::add(text).1);
    draft.set(String::new());
}

#[component]
fn ChecklistRow(checklist: List, id: ObjectId) -> NodeId {
    let item = checklist.object::<ChecklistItem>(id);
    let label = create_memo(move || {
        item.with(|item| {
            item.as_ref()
                .map(|item| item.text.clone())
                .unwrap_or_default()
        })
    });
    let done = create_memo(move || item.with(|item| item.as_ref().is_some_and(|item| item.done)));
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            radius=Radius::Md
            padding={Padding::symmetric(Space::Lg, Space::Md)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Checkbox
                    @sizing=ItemSize::Percent(100.0)
                    label={label}
                    checked={Binding::new(done, move |done| {
                        checklist.operate(ChecklistModel::set_done(id, done));
                    })}
                    @test_id={format!("checklist.item.{id}.done")}
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
