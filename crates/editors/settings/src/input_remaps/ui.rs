use beui::Wrap;
use block_editor_beui::be_block::{InputRemapper, InputRemapperContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::{ContentProjection, Editor, RemapState, RemapsWatch};

use super::gui::EditorTab;
use super::raw::TomlTab;
use super::{MODULE, PLUGIN};

pub(super) type Remapper = ContentProjection<InputRemapperContent>;

#[derive(Clone, Copy)]
pub(super) struct Shared {
    pub(super) remapper: Remapper,
    pub(super) stored: ReadSignal<String>,
    pub(super) read_only: Memo<bool>,
}

impl Shared {
    pub(super) fn store(self, text: &str) {
        if self.read_only.get_untracked() {
            return;
        }
        let config = (!text.trim().is_empty()).then(|| text.as_bytes().to_vec());
        self.remapper
            .operate(InputRemapper::set(PLUGIN, MODULE, config));
    }
}

#[component]
pub fn InputRemapsView(editor: Editor) -> NodeId {
    let remapper = editor.block_content::<InputRemapperContent>();
    let read_only = editor.read_only();
    let device = editor.watch::<RemapsWatch>();
    let offered = editor.offered::<RemapsWatch>();
    let stored = remapper.project(|content| {
        content
            .root()
            .config
            .map(|config| String::from_utf8_lossy(&config).into_owned())
            .unwrap_or_default()
    });
    let shared = Shared {
        remapper,
        stored,
        read_only,
    };
    let tab = create_signal(0usize);
    let editing = create_memo(move || tab.get() == 0);
    let raw = create_memo(move || tab.get() == 1);
    let status = create_memo(move || {
        match device.get() {
            RemapState::Off => "This device runs no remaps.",
            RemapState::On => "This device runs the remaps its input settings pick.",
            RemapState::Stopped => {
                "The remaps on this device stopped, and start again once they change."
            }
        }
        .to_owned()
    });
    view! {
        <SettingsPage>
            <SettingsSection
                title="Input remaps"
                description="Keys, buttons and wheel turns to change in a Linux session, in every program or in the ones a group names. They are kept as TOML text, comments and all, which both tabs edit."
            >
                <SettingsRow label="Edit as" compact=true>
                    <Tabs
                        options={view! {
                            <ChoiceOption label="Editor" />
                            <ChoiceOption label="TOML" />
                        }}
                        selected={tab}
                        @test_id={"input-remaps.tabs"}
                    />
                </SettingsRow>
                <Show condition={offered}>
                    <SettingsRow label="This device" separated=true>
                        <Text style=TextStyle::CAPTION wrap=Wrap::Words>{status}</Text>
                    </SettingsRow>
                </Show>
            </SettingsSection>
            <Show condition={editing}>
                <EditorTab shared={shared} />
            </Show>
            <Show condition={raw}>
                <TomlTab shared={shared} />
            </Show>
        </SettingsPage>
    }
}
