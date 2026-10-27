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
    let report =
        create_memo(move || format!("Remember {} lights {}", remember.get(), lights.get()));
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

fn path_label(path: &[usize]) -> &'static str {
    match path {
        [0] => "Copy",
        [1, 0] => "Email",
        [1, 1] => "Link",
        [2] => "Delete",
        _ => "something else",
    }
}

#[component]
pub fn MenusFixture() -> NodeId {
    let chosen = create_signal(String::from("nothing"));
    let fruit = create_signal(Some(0_usize));
    let report = create_memo(move || format!("Chose {} fruit {:?}", chosen.get(), fruit.get()));
    let choose = move |path: Vec<usize>| chosen.set(path_label(&path).to_owned());
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    label="Actions"
                    items={view! {
                        <MenuItem label="Copy" />
                        <MenuItem label="Share">
                            <MenuItem label="Email" />
                            <MenuItem label="Link" />
                        </MenuItem>
                        <MenuItem label="Delete" separated=true />
                    }}
                    on_select={choose}
                />
                <Select
                    options={view! {
                        <ChoiceOption label="Apple" />
                        <ChoiceOption label="Banana" />
                        <ChoiceOption label="Cherry" />
                    }}
                    selected={fruit}
                    label="Fruit"
                    @test_id="menus.fruit"
                />
            </List>
            <ContextMenu
                items={view! {
                    <MenuItem label="Copy" />
                    <MenuItem label="Share">
                        <MenuItem label="Email" />
                        <MenuItem label="Link" />
                    </MenuItem>
                    <MenuItem label="Delete" separated=true />
                }}
                on_select={choose}
            >
                <Card @test_id="menus.card">
                    <Text>"Right-click here"</Text>
                </Card>
            </ContextMenu>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn PopupsFixture() -> NodeId {
    let starred = create_signal(false);
    let report = create_memo(move || format!("Starred {}", starred.get()));
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Popover label="Filters" @test_id="popups.filters">|handle: PopoverHandle| {
                    let close = handle.close;
                    view! {
                        <List spacing=Space::Md>
                            <Checkbox label="Only starred" checked={starred} />
                            <Button label="Done" on_click={move || close.call(())} />
                        </List>
                    }
                }</Popover>
                <Tooltip label="Says more">
                    <Button label="Hint" @test_id="popups.hint" />
                </Tooltip>
            </List>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn FieldsFixture() -> NodeId {
    let name = create_signal(String::new());
    let seats = create_signal(4.0_f64);
    let load = create_signal(0.5_f32);
    let report = create_memo(move || {
        format!(
            "Name {:?} seats {} load {:.1}",
            name.get(),
            seats.get(),
            load.get()
        )
    });
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(240.0)>
                <TextInput value={name} placeholder="Type a name" @test_id="fields.name" />
            </Frame>
            <Frame width=Size::Custom(240.0)>
                <NumberInput value={seats} min=1.0 max=12.0 label="Seats" @test_id="fields.seats" />
            </Frame>
            <Frame width=Size::Custom(240.0)>
                <Slider value={load} label="Load" />
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn TextAreaFixture() -> NodeId {
    let note = create_signal(String::new());
    let report = create_memo(move || format!("Note {:?}", note.get()));
    view! {
        <List spacing=Space::Md>
            <Frame width=Size::Custom(240.0)>
                <TextInput
                    value={note}
                    placeholder="A note, over several lines"
                    multiline=true
                    @test_id="text_area.note"
                />
            </Frame>
            <Text>{report}</Text>
        </List>
    }
}
