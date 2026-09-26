use beui::NodeId;
use beui::reactive::{
    Align, Callback, Direction, ItemSize, List, Prop, clone, component, create_memo, view,
};
use beui::styled::{Caption, NumberInput};
use block_ui::datetime::{DateTimeFields, days_in_month};

const SPACING: f32 = 6.0;
const YEAR_WIDTH: f32 = 72.0;
const PART_WIDTH: f32 = 56.0;

#[component]
pub fn DateTimeRow(
    value: Prop<DateTimeFields>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<DateTimeFields>,
) -> NodeId {
    let fields = create_memo(move || value.get());
    let year = create_memo(clone!(fields -> move || f64::from(fields.get().year)));
    let month = create_memo(clone!(fields -> move || f64::from(fields.get().month)));
    let day = create_memo(clone!(fields -> move || f64::from(fields.get().day)));
    let hour = create_memo(clone!(fields -> move || f64::from(fields.get().hour)));
    let minute = create_memo(clone!(fields -> move || f64::from(fields.get().minute)));
    let max_day = create_memo(clone!(fields -> move || {
        let fields = fields.get();
        f64::from(days_in_month(fields.year, fields.month.clamp(1, 12)))
    }));

    let edit = move |change: fn(&mut DateTimeFields, f64)| {
        let fields = fields.clone();
        let on_change = on_change.clone();
        move |typed: f64| {
            let mut next = fields.get_untracked();
            change(&mut next, typed);
            next.month = next.month.clamp(1, 12);
            next.day = next.day.clamp(1, days_in_month(next.year, next.month));
            on_change.call(next);
        }
    };
    let set_year = edit(|fields, value| fields.year = value as i32);
    let set_month = edit(|fields, value| fields.month = value as u8);
    let set_day = edit(|fields, value| fields.day = value as u8);
    let set_hour = edit(|fields, value| fields.hour = value as u8);
    let set_minute = edit(|fields, value| fields.minute = value as u8);
    let days = create_memo(clone!(max_day -> move || max_day.get()));
    let off = create_memo(move || disabled.get());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <NumberInput
                @sizing=ItemSize::Fixed(YEAR_WIDTH)
                value={year}
                min=1.0
                max=9999.0
                label="Year"
                disabled={off.clone()}
                on_change={set_year}
            />
            <NumberInput
                @sizing=ItemSize::Fixed(PART_WIDTH)
                value={month}
                min=1.0
                max=12.0
                label="Month"
                disabled={off.clone()}
                on_change={set_month}
            />
            <DayInput
                @sizing=ItemSize::Fixed(PART_WIDTH)
                value={day}
                max={days}
                disabled={off.clone()}
                on_change={set_day}
            />
            <Caption content="at" />
            <NumberInput
                @sizing=ItemSize::Fixed(PART_WIDTH)
                value={hour}
                min=0.0
                max=23.0
                label="Hour"
                disabled={off.clone()}
                on_change={set_hour}
            />
            <Caption content=":" />
            <NumberInput
                @sizing=ItemSize::Fixed(PART_WIDTH)
                value={minute}
                min=0.0
                max=59.0
                label="Minute"
                disabled={off.clone()}
                on_change={set_minute}
            />
        </List>
    }
}

#[component]
fn DayInput(
    value: Prop<f64>,
    max: Prop<f64>,
    disabled: Prop<bool>,
    on_change: Callback<f64>,
) -> NodeId {
    let limit = create_memo(move || max.get());
    let clamped = move |typed: f64| on_change.call(typed.min(limit.get_untracked()).max(1.0));
    view! {
        <NumberInput
            value={value}
            min=1.0
            max=31.0
            label="Day"
            disabled={disabled}
            on_change={clamped}
        />
    }
}
