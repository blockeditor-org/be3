use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Direction};

#[component]
pub fn ButtonsFixture() -> NodeId {
    let clicks = create_signal(0_u32);
    let pinned = create_signal(false);
    let clicked = create_memo(move || format!("Clicked {}", clicks.get()));
    let pin = create_memo(move || format!("Pinned {}", pinned.get()));
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button
                    label="Save"
                    variant=ButtonVariant::Primary
                    @test_id="buttons.save"
                    on_click={move || clicks.set(clicks.get_untracked() + 1)}
                />
                <Button
                    label="Off"
                    disabled=true
                    @test_id="buttons.off"
                    on_click={move || clicks.set(100)}
                />
                <ToggleButton label="Pin" pressed={pinned} />
            </List>
            <Text>{clicked}</Text>
            <Text>{pin}</Text>
        </List>
    }
}

#[component]
pub fn TogglesFixture() -> NodeId {
    let remember = create_signal(false);
    let lights = create_signal(true);
    let report = create_memo(move || format!("Remember {} lights {}", remember.get(), lights.get()));
    view! {
        <List spacing=Space::Md>
            <Checkbox label="Remember me" checked={remember} />
            <Switch label="Lights" on={lights} />
            <Checkbox label="Locked" checked=true disabled=true />
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn ChoicesFixture() -> NodeId {
    let mode = create_signal(Some(0_usize));
    let period = create_signal(1_usize);
    let tab = create_signal(0_usize);
    let color = create_signal(Some(1_usize));
    let report = create_memo(move || {
        format!(
            "Mode {:?} period {} tab {} color {:?}",
            mode.get(),
            period.get(),
            tab.get(),
            color.get()
        )
    });
    view! {
        <List spacing=Space::Md>
            <RadioGroup
                options={view! {
                    <ChoiceOption label="Automatic" />
                    <ChoiceOption label="Manual" />
                    <ChoiceOption label="Scheduled" />
                }}
                selected={mode}
            />
            <SegmentedControl
                label="Period"
                options={view! {
                    <ChoiceOption label="Day" />
                    <ChoiceOption label="Week" />
                    <ChoiceOption label="Month" />
                }}
                selected={period}
            />
            <Tabs
                label="Section"
                options={view! {
                    <ChoiceOption label="Overview" />
                    <ChoiceOption label="Details" />
                }}
                selected={tab}
            />
            <Listbox
                options={view! {
                    <ChoiceOption label="Amber" />
                    <ChoiceOption label="Blue" />
                    <ChoiceOption label="Green" />
                }}
                selected={color}
            />
            <Text>{report}</Text>
        </List>
    }
}
