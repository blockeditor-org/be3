use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Direction};

#[component]
pub fn DialogsFixture() -> NodeId {
    let open = create_signal(false);
    let sheet = create_signal(false);
    let outcome = create_signal(String::from("nothing"));
    let name = create_signal(String::from("Notes"));
    let report = create_memo(move || format!("Outcome {} name {}", outcome.get(), name.get()));
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button label="Rename" on_click={move || open.set(true)} />
                <Button label="Pick" on_click={move || sheet.set(true)} />
            </List>
            <Text>{report}</Text>
            <Dialog
                open={open}
                title="Rename"
                id="dialogs.rename"
                actions={vec![
                    DialogAction::cancel("Cancel"),
                    DialogAction::primary("Save", move || {
                        outcome.set("saved".to_owned());
                        open.set(false);
                    }),
                ]}
                on_dismiss={move || {
                    outcome.set("dismissed".to_owned());
                    open.set(false);
                }}
            >
                <TextInput value={name} label="Name" @test_id="dialogs.name" />
            </Dialog>
            <ModalSheet open={sheet} rest=0.5 on_close={move || sheet.set(false)}>
                <List spacing=Space::None>
                    <ForEach keys={(1..=3).collect::<Vec<usize>>()}>|row: usize|
                        <ActionRow
                            label={format!("Row {row}")}
                            on_click={move || {
                                outcome.set(format!("row {row}"));
                                sheet.set(false);
                            }}
                        />
                    </ForEach>
                </List>
            </ModalSheet>
        </List>
    }
}

#[component]
pub fn ToastsFixture() -> NodeId {
    let area = NodeRef::new();
    let toasts = create_signal(Vec::<Toast>::new());
    let next = create_signal(1_u64);
    let chosen = create_signal(String::from("nothing"));
    let report =
        create_memo(move || format!("Toasts {} chose {}", toasts.with(Vec::len), chosen.get()));
    view! {
        <Frame @node_ref=&area height=Size::Custom(300.0)>
            <List spacing=Space::Md>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Button
                        label="Toast"
                        on_click={move || {
                            let id = next.get_untracked();
                            next.set(id + 1);
                            toasts.update(|toasts| toasts.push(Toast {
                                id,
                                message: format!("Toast number {id}"),
                                ..Toast::default()
                            }));
                        }}
                    />
                    <Button
                        label="Message"
                        on_click={move || {
                            let id = next.get_untracked();
                            next.set(id + 1);
                            toasts.update(|toasts| toasts.push(Toast {
                                id,
                                title: "Mia".to_owned(),
                                message: format!("Message number {id}"),
                                actions: vec![ToastAction {
                                    key: "reply".to_owned(),
                                    label: "Reply".to_owned(),
                                }],
                                sticky: true,
                                ..Toast::default()
                            }));
                        }}
                    />
                </List>
                <Text>{report}</Text>
                <Toasts
                    anchor={area}
                    toasts={toasts}
                    on_dismiss={move |id: u64| toasts.update(|toasts| toasts.retain(|toast| toast.id != id))}
                    on_action={move |(id, key): (u64, String)| {
                        chosen.set(format!("{key} {id}"));
                        toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                    }}
                />
            </List>
        </Frame>
    }
}

fn program(key: &str, title: &str) -> PaletteItem {
    PaletteItem {
        key: key.to_owned(),
        title: title.to_owned(),
        ..PaletteItem::default()
    }
}

#[component]
pub fn PaletteFixture() -> NodeId {
    let open = create_signal(false);
    let outcome = create_signal(String::from("nothing"));
    let report = create_memo(move || format!("Launched {}", outcome.get()));
    let items = vec![
        program("calculator", "Calculator"),
        program("files", "Files"),
        program("terminal", "Terminal"),
    ];
    view! {
        <List spacing=Space::Md>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Button label="Launch" on_click={move || open.set(true)} />
            </List>
            <Text>{report}</Text>
            <CommandPalette
                open={open}
                items={PaletteItems::list(items)}
                placeholder="Search programs"
                label="Search programs"
                id="palette"
                on_choose={move |key: String| outcome.set(key)}
            />
        </List>
    }
}
