use be_sdk_beui::beui::styled::Root;
use std::time::Duration;

use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::{NodeId, Vec2};
use be_sdk_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use be_type_audio::{AudioContent, AudioFile, BlobFile};

mod ui;

use ui::AudioView;

const INTRINSIC_SIZE: Vec2 = Vec2::new(320.0, 180.0);

pub struct AudioRoot;

#[component]
impl be_sdk_beui::BeuiRoot for AudioRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <AudioView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Creation(creation: Creation) -> NodeId {
        view! {
            <ContentFileCreation
                creation
                title="New Audio"
                id_prefix="audio"
                filter={filter()}
                import={decode}
            />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

pub(crate) fn format_micros(micros: u64) -> String {
    format_duration(Duration::from_micros(micros))
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

pub(crate) fn filter() -> FileFilter {
    FileFilter::new("Audio", AudioFile::FILE_EXTENSIONS, AudioFile::MIME_TYPES)
}

pub(crate) fn decode(file: PickedFile) -> Result<AudioContent, String> {
    let PickedFile { name, data } = file;
    if data.is_empty() {
        return Err(format!("Could not import {name}: it is empty"));
    }
    Ok(AudioFile::from_file(name, data))
}
