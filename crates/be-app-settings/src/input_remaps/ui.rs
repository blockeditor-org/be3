use be_block_input::{InputRemapper, InputRemapperContent};
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{ContentProjection, Editor, RemapState, RemapsWatch};

use super::gui::EditorTab;
use super::raw::TomlTab;
use super::{APP, MODULE};

pub(super) type Remapper = ContentProjection<InputRemapperContent>;

#[derive(Clone, Copy)]
pub(super) struct Shared {
    pub(super) remapper: Remapper,
    pub(super) stored: Memo<String>,
    pub(super) read_only: Memo<bool>,
}

impl Shared {
    pub(super) fn store(self, text: &str) {
        if self.read_only.get_untracked() {
            return;
        }
        let config = (!text.trim().is_empty()).then(|| text.as_bytes().to_vec());
        self.remapper
            .operate(InputRemapper::set(APP, MODULE, config));
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
    let toml = create_signal(false);
    let status = create_memo(move || {
        match device.get() {
            RemapState::Off => "This device runs no remaps.",
            RemapState::On => "Running on this device.",
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
                description="Change what keys, mouse buttons and the wheel do in a Linux session, everywhere or only in the programs you choose."
            >
                <Show value={offered}>
                    <SettingsRow label="This device">
                        <Text style=TextStyle::CAPTION>{status}</Text>
                    </SettingsRow>
                </Show>
                <SettingsRow
                    label="Edit as TOML"
                    description="The remaps are kept as TOML text, comments and all."
                    separated={offered}
                    compact=true
                >
                    <Switch on={toml} label="Edit as TOML" @test_id={"input-remaps.toml"} />
                </SettingsRow>
            </SettingsSection>
            <Show
                value={toml}
                fallback={move || view! {
                    <EditorTab shared={shared} editor={editor} />
                }}
            >
                <TomlTab shared={shared} />
            </Show>
        </SettingsPage>
    }
}
