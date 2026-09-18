use std::cell::RefCell;
use std::rc::Rc;

use block_client::blocks::game_module::{GameModule, GameModuleOperation};
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, NodeRef, Show, clone, component, create_memo,
    create_signal, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Heading, Paragraph, use_theme,
};
use block_editor_plugin::beui::{NodeId, TextAlign};
use block_editor_plugin::{Creation, Editor, FilePicker, Sidebar};
use game_host::Game;

use super::{filter, imported};

const PADDING: f32 = 12.0;
const SPACING: f32 = 8.0;

#[derive(Clone, Default, PartialEq)]
enum Loaded {
    #[default]
    Loading,
    Named(String),
    Failed(String),
}

#[component]
pub fn ModuleView(editor: Editor) -> NodeId {
    let module = editor.block::<GameModule>();
    let loaded = module.project(|module| match Game::load(module.data()) {
        Ok(game) => Loaded::Named(game.name().to_owned()),
        Err(error) => Loaded::Failed(error),
    });
    let source = module.project(|module| module.source_name().to_owned());
    let size = module.project(|module| module.data().len());
    let bytes = create_memo(clone!(size -> move || format!("{} bytes", size.get())));

    let name = create_memo(clone!(loaded -> move || match loaded.get() {
        Loaded::Named(name) => name,
        _ => String::new(),
    }));
    let named = create_memo(clone!(loaded -> move || matches!(loaded.get(), Loaded::Named(_))));
    let reason = create_memo(clone!(loaded -> move || match loaded.get() {
        Loaded::Failed(error) => error,
        _ => String::new(),
    }));
    let failed = create_memo(clone!(loaded -> move || matches!(loaded.get(), Loaded::Failed(_))));
    let waiting = create_memo(clone!(loaded -> move || loaded.get() == Loaded::Loading));

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let panel = editor.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @node_ref={&content}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <List spacing=SPACING>
                        <Show condition={named}>
                            <Heading
                                content={name}
                                align=TextAlign::Center
                                @test_id={"game-module.name"}
                            />
                        </Show>
                        <Show condition={failed}>
                            <Paragraph
                                content={reason}
                                align=TextAlign::Center
                                color={theme.danger.clone()}
                                @test_id={"game-module.error"}
                            />
                        </Show>
                        <Show condition={waiting}>
                            <Caption content="Loading…" align=TextAlign::Center />
                        </Show>
                        <Body content={source} align=TextAlign::Center />
                        <Caption content={bytes} align=TextAlign::Center />
                    </List>
                </Frame>
                <Sidebar shown={chrome}>
                    <ModulePanel editor={panel} />
                </Sidebar>
            </List>
        </Frame>
    }
}

#[component]
fn ModulePanel(editor: Editor) -> NodeId {
    let module = editor.block::<GameModule>();
    let picker = Rc::new(RefCell::new(FilePicker::default()));
    let (error, set_error) = create_signal(None::<String>);
    let (picking, set_picking) = create_signal(false);
    let host = editor.host().clone();
    let polled = Rc::clone(&picker);
    editor.each_frame(clone!(module -> move || {
        let picked = polled.borrow_mut().poll(&host).map(|file| file.and_then(imported));
        set_picking.set(polled.borrow().is_open());
        match picked {
            Some(Ok(module_block)) => {
                module.operate(GameModuleOperation::Replace {
                    module: module_block,
                });
                set_error.set(None);
            }
            Some(Err(reason)) => set_error.set(Some(reason)),
            None => {}
        }
    }));

    let read_only = editor.read_only();
    let blocked =
        create_memo(clone!(picking read_only -> move || picking.get() || read_only.get()));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let opened = editor.host().clone();
    let replace = move || picker.borrow_mut().open(&opened, filter());
    let theme = use_theme();
    view! {
        <List spacing=SPACING>
            <Heading content="Game module" />
            <Button
                label="Replace module..."
                variant=ButtonVariant::Secondary
                disabled={blocked}
                @test_id={"game-module.replace"}
                on_click={replace}
            />
            <Show condition={failed}>
                <Caption content={reason} color={theme.danger.clone()} />
            </Show>
        </List>
    }
}

#[component]
pub fn ModuleCreation(creation: Creation) -> NodeId {
    let picker = Rc::new(RefCell::new(FilePicker::default()));
    let chosen = Rc::new(RefCell::new(None::<GameModule>));
    let (name, set_name) = create_signal(None::<String>);
    let (error, set_error) = create_signal(None::<String>);
    let (picking, set_picking) = create_signal(false);
    creation.set_ready(false);

    let host = creation.host().clone();
    let polled = Rc::clone(&picker);
    let filled = Rc::clone(&chosen);
    let ready = creation.clone();
    creation.each_frame(move || {
        let picked = polled
            .borrow_mut()
            .poll(&host)
            .map(|file| file.and_then(imported));
        set_picking.set(polled.borrow().is_open());
        match picked {
            Some(Ok(module)) => {
                set_name.set(Some(module.source_name().to_owned()));
                *filled.borrow_mut() = Some(module);
                set_error.set(None);
                ready.set_ready(true);
            }
            Some(Err(reason)) => {
                *filled.borrow_mut() = None;
                set_name.set(None);
                set_error.set(Some(reason));
                ready.set_ready(false);
            }
            None => {}
        }
    });

    let client = creation.client().clone();
    let made = Rc::clone(&chosen);
    creation.on_create(move || {
        let module = made.borrow_mut().take().ok_or("no file was chosen")?;
        Ok(client.create_block(module).id())
    });

    let opened = creation.host().clone();
    let choose = move || picker.borrow_mut().open(&opened, filter());
    let label = create_memo(clone!(name -> move || {
        name.get().unwrap_or_else(|| "No file chosen".to_owned())
    }));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        label="Choose file..."
                        variant=ButtonVariant::Secondary
                        disabled={picking}
                        @test_id={"game-module.choose"}
                        on_click={choose}
                    />
                    <Caption @sizing=ItemSize::Percent(100.0) content={label} />
                </List>
                <Show condition={failed}>
                    <Caption
                        content={reason}
                        color={theme.danger.clone()}
                        @test_id={"game-module.error"}
                    />
                </Show>
            </List>
        </Frame>
    }
}
