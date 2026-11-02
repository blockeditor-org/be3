use be_sdk_beui::beui::icons::ICON_KEYBOARD;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Key, NodeId};
use be_sdk_beui::desktop;
use be_sdk_beui::{Editor, KeyboardLayouts, KeyboardLayoutsWatch};
use beui::KeyAt;

use super::bar::bar_bounds;

#[derive(Clone, Copy)]
pub(crate) struct Layouts {
    editor: Editor,
    pub(crate) listed: Memo<KeyboardLayouts>,
    pub(crate) several: Memo<bool>,
}

impl Layouts {
    pub(crate) fn new(editor: Editor) -> Self {
        let listed = editor.watch::<KeyboardLayoutsWatch>();
        let several = create_memo(move || listed.with(|listed| listed.layouts.len() > 1));
        Self {
            editor,
            listed,
            several,
        }
    }

    fn switch(self, index: u32) {
        self.editor
            .act(move || desktop::input::switch_layout(index).detach());
    }

    fn next(self) {
        let next = self.listed.with_untracked(|listed| {
            let count = u32::try_from(listed.layouts.len()).unwrap_or(u32::MAX);
            (count > 1).then(|| (listed.active + 1) % count)
        });
        if let Some(next) = next {
            self.switch(next);
        }
    }
}

#[component]
pub(crate) fn LayoutKey(layouts: Layouts) -> NodeId {
    Action::new("desktop.layout.next", "Next keyboard layout", move || {
        layouts.next();
    })
    .glyph(ICON_KEYBOARD)
    .shortcut(Chord::logo(Key::Space))
    .at(KeyAt::Window)
    .capture()
    .register();
    view! {
        <Frame />
    }
}

#[component]
pub(crate) fn LayoutButton(layouts: Layouts) -> NodeId {
    let listed = layouts.listed;
    let label = create_memo(move || {
        listed.with(|listed| {
            listed
                .layouts
                .get(listed.active as usize)
                .map(|layout| layout.label.clone())
                .unwrap_or_default()
        })
    });
    view! {
        <MenuButton
            variant=ButtonVariant::Ghost
            size=ButtonSize::Compact
            arrow=false
            label
            bounds={bar_bounds()}
            @test_id={"desktop.layout"}
            items={view! {
                <LayoutItems layouts />
            }}
        />
    }
}

#[component]
fn LayoutItems(layouts: Layouts) -> DynamicSegment<MenuItem> {
    let listed = layouts.listed;
    let keys = create_memo(move || {
        listed.with(|listed| (0..listed.layouts.len() as u32).collect::<Vec<u32>>())
    });
    view! {
        <ForEach keys>|index: u32|
            <LayoutItem layouts index />
        </ForEach>
    }
}

#[component]
fn LayoutItem(layouts: Layouts, index: u32) -> MenuItem {
    let listed = layouts.listed;
    let name = create_memo(move || {
        listed.with(|listed| {
            listed
                .layouts
                .get(index as usize)
                .map(|layout| layout.name.clone())
                .unwrap_or_default()
        })
    });
    let checked = create_memo(move || Some(listed.with(|listed| listed.active == index)));
    view! {
        <MenuItem
            label={name}
            checked
            radio=true
            row_test_id={format!("desktop.layout.{index}")}
            on_click={move || layouts.switch(index)}
        />
    }
}
