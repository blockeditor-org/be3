use block_editor_beui::beui::datetime::DateTime;
use block_editor_beui::beui::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_PLUS};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{Editor, Toolbar};
use calendar_types::{Calendar, CalendarContent};

use super::model::{
    CalendarView, EventForm, FormAction, Shown, today_days_since_epoch, week_start,
};
use super::month::MonthGrid;
use super::timeline::Timeline;

const PADDING: f32 = 12.0;
const SPACING: f32 = 8.0;
const INTRINSIC_WIDTH: f32 = 880.0;
const MONTH_HEIGHT: f32 = 600.0;
const TIMELINE_HEIGHT: f32 = 660.0;
const FORM_WIDTH: f32 = 420.0;

#[component]
pub fn CalendarEditor(editor: Editor) -> NodeId {
    let calendar = editor.block_content::<CalendarContent>();
    let events = calendar.project(|calendar| {
        let mut events = calendar.root().events.to_vec();
        events.sort_by_key(|event| event.start);
        events
    });
    let events = create_memo(move || events.get());
    let mode = create_signal(CalendarView::default());
    let anchor = create_signal(today_days_since_epoch());
    let form = create_signal(None::<EventForm>);

    create_effect(move || {
        let height = match mode.get() {
            CalendarView::Month => MONTH_HEIGHT,
            CalendarView::Day | CalendarView::Week => TIMELINE_HEIGHT,
        };
        editor.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    });

    let read_only = editor.read_only();
    let header = create_memo(move || mode.get().header(anchor.get()));
    let mode_index = create_memo(move || {
        CalendarView::ALL
            .iter()
            .position(|shown| *shown == mode.get())
            .unwrap_or(0)
    });
    let previous = move || {
        anchor.set(mode.get_untracked().previous(anchor.get_untracked()));
    };
    let next = move || {
        anchor.set(mode.get_untracked().next(anchor.get_untracked()));
    };
    let today = move || anchor.set(today_days_since_epoch());
    let add = move || {
        form.set(Some(EventForm::new_at(anchor.get_untracked(), 9)));
    };

    let editable = editor.editable();
    let operate = move |action: FormAction| {
        if !editable.get_untracked() {
            return;
        }
        let edit = match action {
            FormAction::Save(Some(id), event) => calendar
                .read(|content| content.root().update(id, &event))
                .unwrap_or_default(),
            FormAction::Save(None, event) => Calendar::add(&event).1,
            FormAction::Delete(id) => Calendar::remove(id),
        };
        if !edit.0.is_empty() {
            calendar.operate(edit);
        }
    };
    let pick_day = move |day: i64| {
        if editable.get_untracked() {
            form.set(Some(EventForm::new_at(day, 9)));
        }
    };
    let pick_slot = move |(day, hour): (i64, u8)| {
        if editable.get_untracked() {
            form.set(Some(EventForm::new_at(day, hour)));
        }
    };
    let pick_event = move |event: Shown| form.set(Some(EventForm::edit(&event)));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    view! {
        <List spacing=0.0>
            <Toolbar shown={chrome}>
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_LEFT
                    label="Previous"
                    @test_id={"calendar.previous"}
                    on_click={previous}
                />
                <Button
                    label="Today"
                    variant=ButtonVariant::Secondary
                    @test_id={"calendar.today"}
                    on_click={today}
                />
                <Button
                    icon_only=true
                    glyph=ICON_CHEVRON_RIGHT
                    label="Next"
                    @test_id={"calendar.next"}
                    on_click={next}
                />
                <Text>{header}</Text>
                <Tabs
                    options={view! {
                        <ChoiceOption label={CalendarView::Day.label()} />
                        <ChoiceOption label={CalendarView::Week.label()} />
                        <ChoiceOption label={CalendarView::Month.label()} />
                    }}
                    selected={Binding::new(mode_index, move |chosen: usize| {
                        if let Some(shown) = CalendarView::ALL.get(chosen) {
                            mode.set(*shown);
                        }
                    })}
                    @test_id={"calendar.layout"}
                />
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Button
                    glyph=ICON_PLUS
                    label="Event"
                    variant=ButtonVariant::Primary
                    disabled={read_only}
                    @test_id={"calendar.add-event"}
                    on_click={add}
                />
            </Toolbar>
            <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&content} padding=PADDING>
                <List spacing=0.0>
                    <Dynamic value={mode}>|shown: CalendarView| {
                        match shown {
                            CalendarView::Month => view! {
                                <MonthGrid
                                    anchor={create_memo(move || anchor.get())}
                                    events={events}
                                    on_pick_day={pick_day}
                                    on_pick_event={pick_event}
                                />
                            },
                            CalendarView::Week => {
                                let first = create_memo(move || week_start(anchor.get()));
                                view! {
                                    <Timeline
                                        first_day={first}
                                        days=7
                                        events={events}
                                        on_pick_slot={pick_slot}
                                        on_pick_event={pick_event}
                                    />
                                }
                            }
                            CalendarView::Day => view! {
                                <Timeline
                                    first_day={create_memo(move || anchor.get())}
                                    days=1
                                    events={events}
                                    on_pick_slot={pick_slot}
                                    on_pick_event={pick_event}
                                />
                            },
                        }
                    }</Dynamic>
                </List>
            </Frame>
            <EventDialog form={form} read_only={read_only} on_operate={operate} />
        </List>
    }
}

#[component]
fn EventDialog(
    form: Signal<Option<EventForm>>,
    read_only: block_editor_beui::beui::reactive::Memo<bool>,
    on_operate: block_editor_beui::beui::reactive::Callback<FormAction>,
) -> NodeId {
    let open = create_memo(move || form.get().is_some());
    let title = create_memo(move || {
        match form.with(|form| form.as_ref().is_some_and(|form| form.editing_id.is_some())) {
            true => "Edit event".to_owned(),
            false => "New event".to_owned(),
        }
    });
    let name = create_memo(move || {
        form.with(|form| {
            form.as_ref()
                .map(|form| form.title.clone())
                .unwrap_or_default()
        })
    });
    let start = create_memo(move || form.with(|form| form.as_ref().map(|form| form.start)));
    let end = create_memo(move || form.with(|form| form.as_ref().map(|form| form.end)));
    let backwards = create_memo(move || {
        form.with(|form| form.as_ref().is_some_and(|form| form.end < form.start))
    });
    let editing = create_memo(move || {
        form.with(|form| form.as_ref().is_some_and(|form| form.editing_id.is_some()))
    });
    let blank = create_memo(move || {
        read_only.get()
            || backwards.get()
            || form.with(|form| {
                form.as_ref()
                    .is_none_or(|form| form.title.trim().is_empty())
            })
    });

    let edit_title = move |typed: String| {
        form.update(|form| {
            if let Some(form) = form {
                form.title = typed;
            }
        });
    };
    let edit_start = move |picked: Option<DateTime>| {
        form.update(|form| {
            if let (Some(form), Some(picked)) = (form, picked) {
                form.move_start(picked);
            }
        });
    };
    let edit_end = move |picked: Option<DateTime>| {
        form.update(|form| {
            if let (Some(form), Some(picked)) = (form, picked) {
                form.end = picked;
            }
        });
    };
    let save = move || {
        let Some(shown) = form.get_untracked() else {
            return;
        };
        on_operate.call(FormAction::Save(shown.editing_id, shown.event()));
        form.set(None);
    };
    let delete = move || {
        if let Some(id) = form.get_untracked().and_then(|form| form.editing_id) {
            on_operate.call(FormAction::Delete(id));
        }
        form.set(None);
    };
    let dismiss = move || form.set(None);
    view! {
        <Dialog
            open={open}
            title={title}
            width=FORM_WIDTH
            id="calendar.form"
            confirm="Save"
            cancel="Cancel"
            confirm_disabled={blank}
            on_confirm={save}
            on_dismiss={dismiss}
            extra_actions={move || view! {
                <Show condition={editing}>
                    <Button
                        label="Delete"
                        variant=ButtonVariant::Danger
                        @test_id={"calendar.form.delete"}
                        on_click={delete}
                    />
                </Show>
            }}
        >
            <List spacing=SPACING>
                <TextInput
                    value={Binding::new(name, edit_title)}
                    label="Title"
                    placeholder="Title"
                    disabled={read_only}
                    @test_id={"calendar.form.title"}
                />
                <Text style=TextStyle::CAPTION>"Starts"</Text>
                <DateTimeField
                    value={Binding::new(start, edit_start)}
                    label="Starts"
                    disabled={read_only}
                    @test_id={"calendar.form.start"}
                />
                <Text style=TextStyle::CAPTION>"Ends"</Text>
                <DateTimeField
                    value={Binding::new(end, edit_end)}
                    label="Ends"
                    disabled={read_only}
                    @test_id={"calendar.form.end"}
                />
                <Show condition={backwards}>
                    <Text style=TextStyle::CAPTION>"The event ends before it starts."</Text>
                </Show>
            </List>
        </Dialog>
    }
}
