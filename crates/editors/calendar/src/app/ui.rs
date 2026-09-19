use std::rc::Rc;

use block_client::blocks::calendar::{Calendar, CalendarEvent, CalendarOperation};
use block_editor_plugin::beui::icons::{
    ICON_ADD, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CLOSE, ICON_DELETE, ICON_SAVE,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, Dynamic, Frame, ItemSize, List, NodeRef, Show, Spacer, WriteSignal, clone,
    component, create_effect, create_memo, create_signal, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Dialog, IconButton, Tabs, TextInput, use_theme,
};
use block_editor_plugin::beui::unstyled::ChoiceOption;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{DateTimeRow, Editor, Toolbar};

use super::model::{CalendarView, EventForm, today_days_since_epoch, week_start};
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
    let calendar = editor.block::<Calendar>();
    let events = calendar.project(|calendar| {
        let mut events = calendar.events().to_vec();
        events.sort_by_key(|event| event.start);
        events
    });
    let events = create_memo(clone!(events -> move || events.get()));
    let (mode, set_mode) = create_signal(CalendarView::default());
    let (anchor, set_anchor) = create_signal(today_days_since_epoch());
    let (form, set_form) = create_signal(None::<EventForm>);
    let anchor = create_memo(clone!(anchor -> move || anchor.get()));

    let sized = editor.clone();
    create_effect(clone!(mode -> move || {
        let height = match mode.get() {
            CalendarView::Month => MONTH_HEIGHT,
            CalendarView::Day | CalendarView::Week => TIMELINE_HEIGHT,
        };
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let read_only = editor.read_only();
    let header = create_memo(clone!(mode anchor -> move || mode.get().header(anchor.get())));
    let mode_index = create_memo(clone!(mode -> move || {
        CalendarView::ALL
            .iter()
            .position(|shown| *shown == mode.get())
            .unwrap_or(0)
    }));
    let previous = clone!(mode anchor set_anchor -> move || {
        set_anchor.set(mode.get_untracked().previous(anchor.get_untracked()));
    });
    let next = clone!(mode anchor set_anchor -> move || {
        set_anchor.set(mode.get_untracked().next(anchor.get_untracked()));
    });
    let today = clone!(set_anchor -> move || set_anchor.set(today_days_since_epoch()));
    let add = clone!(anchor set_form -> move || {
        set_form.set(Some(EventForm::new_at(anchor.get_untracked(), 9)));
    });

    let editable = editor.editable();
    let operate = {
        let calendar = Rc::clone(&calendar);
        let editable = editable.clone();
        move |operation: CalendarOperation| {
            if editable.get_untracked() {
                calendar.operate(operation);
            }
        }
    };
    let pick_day = clone!(set_form editable -> move |day: i64| {
        if editable.get_untracked() {
            set_form.set(Some(EventForm::new_at(day, 9)));
        }
    });
    let pick_slot = clone!(set_form editable -> move |(day, hour): (i64, u8)| {
        if editable.get_untracked() {
            set_form.set(Some(EventForm::new_at(day, hour)));
        }
    });
    let pick_event =
        clone!(set_form -> move |event: CalendarEvent| set_form.set(Some(EventForm::edit(&event))));

    let form_read_only = read_only.clone();
    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Toolbar shown={chrome}>
                    <IconButton
                        glyph={ICON_CHEVRON_LEFT.to_owned()}
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
                    <IconButton
                        glyph={ICON_CHEVRON_RIGHT.to_owned()}
                        label="Next"
                        @test_id={"calendar.next"}
                        on_click={next}
                    />
                    <Body content={header} />
                    <Tabs
                        options={view! {
                            <ChoiceOption label={CalendarView::Day.label()} />
                            <ChoiceOption label={CalendarView::Week.label()} />
                            <ChoiceOption label={CalendarView::Month.label()} />
                        }}
                        selected={mode_index}
                        @test_id={"calendar.layout"}
                        on_change={move |chosen: usize| {
                            if let Some(shown) = CalendarView::ALL.get(chosen) {
                                set_mode.set(*shown);
                            }
                        }}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button
                        glyph={ICON_ADD.to_owned()}
                        label="Event"
                        variant=ButtonVariant::Primary
                        disabled={read_only.clone()}
                        @test_id={"calendar.add-event"}
                        on_click={add}
                    />
                </Toolbar>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <List spacing=0.0>
                        <Dynamic value={mode}>
                            {move |shown: CalendarView| {
                                let anchor = anchor.clone();
                                let events = events.clone();
                                let pick_day = pick_day.clone();
                                let pick_slot = pick_slot.clone();
                                let pick_event = pick_event.clone();
                                match shown {
                                    CalendarView::Month => view! {
                                        <MonthGrid
                                            anchor={anchor}
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
                                            first_day={anchor}
                                            days=1
                                            events={events}
                                            on_pick_slot={pick_slot}
                                            on_pick_event={pick_event}
                                        />
                                    },
                                }
                            }}
                        </Dynamic>
                    </List>
                </Frame>
                <EventDialog
                    form={form}
                    set_form={set_form}
                    read_only={form_read_only}
                    on_operate={operate}
                />
            </List>
        </Frame>
    }
}

#[component]
fn EventDialog(
    form: block_editor_plugin::beui::reactive::ReadSignal<Option<EventForm>>,
    set_form: WriteSignal<Option<EventForm>>,
    read_only: block_editor_plugin::beui::reactive::Memo<bool>,
    on_operate: block_editor_plugin::beui::reactive::Callback<CalendarOperation>,
) -> NodeId {
    let open = create_memo(clone!(form -> move || form.get().is_some()));
    let title = create_memo(clone!(form -> move || {
        match form.with(|form| form.as_ref().is_some_and(|form| form.editing_id.is_some())) {
            true => "Edit event".to_owned(),
            false => "New event".to_owned(),
        }
    }));
    let name = create_memo(clone!(form -> move || {
        form.with(|form| form.as_ref().map(|form| form.title.clone()).unwrap_or_default())
    }));
    let start = create_memo(clone!(form -> move || {
        form.with(|form| form.as_ref().map(|form| form.start)).unwrap_or_else(|| {
            EventForm::new_at(today_days_since_epoch(), 9).start
        })
    }));
    let end = create_memo(clone!(form -> move || {
        form.with(|form| form.as_ref().map(|form| form.end)).unwrap_or_else(|| {
            EventForm::new_at(today_days_since_epoch(), 9).end
        })
    }));
    let editing = create_memo(clone!(form -> move || {
        form.with(|form| form.as_ref().is_some_and(|form| form.editing_id.is_some()))
    }));
    let blank = create_memo(clone!(form read_only -> move || {
        read_only.get()
            || form.with(|form| {
                form.as_ref().is_none_or(|form| form.title.trim().is_empty())
            })
    }));

    let edit_title = clone!(set_form -> move |typed: String| {
        set_form.update(|form| {
            if let Some(form) = form {
                form.title = typed;
            }
        });
    });
    let edit_start = clone!(set_form -> move |fields| {
        set_form.update(|form| {
            if let Some(form) = form {
                form.start = fields;
            }
        });
    });
    let edit_end = clone!(set_form -> move |fields| {
        set_form.update(|form| {
            if let Some(form) = form {
                form.end = fields;
            }
        });
    });
    let save = clone!(form set_form on_operate -> move || {
        let Some(shown) = form.get_untracked() else {
            return;
        };
        let event = shown.event();
        let operation = match shown.editing_id.is_some() {
            true => CalendarOperation::UpdateEvent { event },
            false => CalendarOperation::AddEvent { event },
        };
        on_operate.call(operation);
        set_form.set(None);
    });
    let delete = clone!(form set_form on_operate -> move || {
        if let Some(id) = form.get_untracked().and_then(|form| form.editing_id) {
            on_operate.call(CalendarOperation::RemoveEvent { id });
        }
        set_form.set(None);
    });
    let cancel = clone!(set_form -> move || set_form.set(None));
    let dismiss = clone!(set_form -> move || set_form.set(None));
    view! {
        <Dialog open={open} title={title} width=FORM_WIDTH on_dismiss={dismiss}>
            <List spacing=SPACING>
                <TextInput
                    value={name}
                    label="Title"
                    placeholder="Title"
                    disabled={read_only.clone()}
                    @test_id={"calendar.form.title"}
                    on_change={edit_title}
                />
                <Caption content="Start" />
                <DateTimeRow value={start} disabled={read_only.clone()} on_change={edit_start} />
                <Caption content="End" />
                <DateTimeRow value={end} disabled={read_only} on_change={edit_end} />
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        glyph={ICON_SAVE.to_owned()}
                        label="Save"
                        variant=ButtonVariant::Primary
                        disabled={blank}
                        @test_id={"calendar.form.save"}
                        on_click={save}
                    />
                    <Show condition={editing}>
                        <Button
                            glyph={ICON_DELETE.to_owned()}
                            label="Delete"
                            variant=ButtonVariant::Secondary
                            @test_id={"calendar.form.delete"}
                            on_click={delete}
                        />
                    </Show>
                    <Button
                        glyph={ICON_CLOSE.to_owned()}
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        @test_id={"calendar.form.cancel"}
                        on_click={cancel}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Dialog>
    }
}
