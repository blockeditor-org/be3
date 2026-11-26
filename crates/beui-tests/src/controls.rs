use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Direction};
use beui_text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

#[component]
pub fn ButtonsFixture() -> NodeId {
    let clicks = create_signal(0_u32);
    let pinned = create_signal(false);
    let clicked = create_memo(move || format!("Clicked {}", clicks.get()));
    let pin = create_memo(move || format!("Pinned {}", pinned.get()));
    view! {
        <List spacing=Space::Md>
            <Text style=TextStyle::HEADING>"Actions"</Text>
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
    let mode = create_signal(Some("Automatic"));
    let period = create_signal("Week");
    let tab = create_signal("Overview");
    let report = create_memo(move || {
        format!(
            "Mode {} period {} tab {}",
            mode.get().unwrap_or("none"),
            period.get(),
            tab.get()
        )
    });
    view! {
        <List spacing=Space::Md>
            <RadioGroup
                options={view! {
                    <ChoiceOption value="Automatic" label="Automatic" />
                    <ChoiceOption value="Manual" label="Manual" />
                    <ChoiceOption value="Scheduled" label="Scheduled" />
                }}
                selected={mode}
            />
            <SegmentedControl
                label="Period"
                options={view! {
                    <ChoiceOption value="Day" label="Day" />
                    <ChoiceOption value="Week" label="Week" />
                    <ChoiceOption value="Month" label="Month" />
                }}
                selected={period}
            />
            <Tabs
                label="Section"
                options={view! {
                    <ChoiceOption value="Overview" label="Overview" />
                    <ChoiceOption value="Details" label="Details" />
                }}
                selected={tab}
            />
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn MenusFixture() -> NodeId {
    let chosen = create_signal(String::from("nothing"));
    let fruit = create_signal(Some("Apple"));
    let report = create_memo(move || {
        format!(
            "Chose {} fruit {}",
            chosen.get(),
            fruit.get().unwrap_or("none")
        )
    });
    let choose = move |what: &str| chosen.set(what.to_owned());
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    label="Actions"
                    items={view! {
                        <MenuItem label="Copy" on_click={move || choose("Copy")} />
                        <MenuItem label="Share">
                            <MenuItem label="Email" on_click={move || choose("Email")} />
                            <MenuItem label="Link" on_click={move || choose("Link")} />
                        </MenuItem>
                        <MenuItem
                            label="Delete"
                            separated=true
                            on_click={move || choose("Delete")}
                        />
                    }}
                />
                <Select
                    options={view! {
                        <ChoiceOption value="Apple" label="Apple" />
                        <ChoiceOption value="Banana" label="Banana" />
                        <ChoiceOption value="Cherry" label="Cherry" />
                    }}
                    selected={fruit}
                    label="Fruit"
                    @test_id="menus.fruit"
                />
            </List>
            <ContextMenu
                items={view! {
                    <MenuItem label="Copy" on_click={move || choose("Copy")} />
                    <MenuItem label="Share">
                        <MenuItem label="Email" on_click={move || choose("Email")} />
                        <MenuItem label="Link" on_click={move || choose("Link")} />
                    </MenuItem>
                    <MenuItem label="Delete" separated=true on_click={move || choose("Delete")} />
                }}
            >
                <Card @test_id="menus.card">
                    <List spacing=Space::Md>
                        <Text>"Right-click here"</Text>
                        <Button label="Focus me" @test_id="menus.card.focus" />
                    </List>
                </Card>
            </ContextMenu>
            <Text>{report}</Text>
        </List>
    }
}

#[component]
pub fn LongMenusFixture() -> NodeId {
    let chosen = create_signal(Vec::<usize>::new());
    let report = create_memo(move || format!("Chose {:?}", chosen.get()));
    let rows = create_memo(|| (1..=23).collect::<Vec<u32>>());
    let subrows = create_memo(|| (1..=16).collect::<Vec<u32>>());
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <MenuButton
                    label="Long"
                    items={view! {
                        <ForEach value={rows}>|row: u32|
                            <MenuItem
                                label={format!("Item {row}")}
                                on_click={move || chosen.set(vec![row as usize - 1])}
                            />
                        </ForEach>
                        <MenuItem label="Item 24">
                            <ForEach value={subrows}>|row: u32|
                                <MenuItem
                                    label={format!("Sub {row}")}
                                    on_click={move || chosen.set(vec![23, row as usize - 1])}
                                />
                            </ForEach>
                        </MenuItem>
                    }}
                />
            </List>
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

/// A markdown rule's row, its characters sharing the line, is one line tall like any other:
/// the text with a rule is as tall as the text with a one-letter paragraph in its place. A
/// quote's `>` and the space after it take the room of its gutter: the caret before, between
/// and after them stands further right each time.
#[component]
pub fn TextRuleFixture() -> NodeId {
    let area = |text: &'static str, id: &'static str| {
        let document = std::sync::Arc::new(TextBuffer::new(text))
            as std::sync::Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
        let height = create_signal(None::<f32>);
        let layout = state.layout();
        let area = view! {
            <Frame width=Size::Custom(240.0) height=Size::Custom(160.0)>
                <TextArea
                    @test_id={id}
                    state={state}
                    line_numbers=false
                    find=false
                    on_content_size={move |size: beui::Vec2| height.set(Some(size.y))}
                />
            </Frame>
        };
        (area, height, layout)
    };
    let (ruled, rule, _) = area("Above\n\n---\n\nBelow", "text_rule.rule");
    let (plain, letter, _) = area("Above\n\nx\n\nBelow", "text_rule.plain");
    let (quoted, laid, quote) = area("> Quoted\n", "text_rule.quote");
    let steps = create_memo(move || {
        // Its carets are read once its rows are laid out, which its content size follows.
        laid.track();
        let layout = quote.get();
        let x = |at: usize| layout.caret_rect(at).map(|rect| rect.left());
        match (x(0), x(1), x(2)) {
            (Some(before), Some(mark), Some(text)) if before < mark && mark + 4.0 < text => {
                "Quote prefix takes its gutter".to_owned()
            }
            carets => format!("Quote carets {carets:?}"),
        }
    });
    let report = create_memo(move || match (rule.get(), letter.get()) {
        (Some(rule), Some(letter)) if (rule - letter).abs() < 0.5 => "Rule is one line".to_owned(),
        (rule, letter) => format!("Rule {rule:?} letter {letter:?}"),
    });
    view! {
        <List spacing=Space::Md>
            {ruled}
            {plain}
            {quoted}
            <Text>{report}</Text>
            <Text>{steps}</Text>
        </List>
    }
}

#[component]
pub fn DescribedFixture() -> NodeId {
    let renames = create_signal(0_u32);
    let inner = create_memo(move || format!("Inner {}", renames.get()));
    let report = create_memo(move || format!("Renamed {}", renames.get()));
    view! {
        <List spacing=Space::Md>
            <DescribedOuter inner />
            <Button
                label="Rename"
                @test_id="described.rename"
                on_click={move || renames.update(|renames| *renames += 1)}
            />
            <Text>{report}</Text>
        </List>
    }
}

fn group(label: String) -> beui::accesskit::Node {
    let mut node = beui::accesskit::Node::new(beui::accesskit::Role::Group);
    node.set_label(label);
    node
}

#[component]
fn DescribedOuter(inner: Memo<String>) -> NodeId {
    component_accessibility(group("Outer".to_owned()));
    view! {
        <DescribedInner label={inner} />
    }
}

#[component]
fn DescribedInner(label: Memo<String>) -> NodeId {
    component_accessibility(create_memo(move || group(label.get())));
    view! {
        <List spacing=Space::Md>
            <Text>"Described"</Text>
        </List>
    }
}
