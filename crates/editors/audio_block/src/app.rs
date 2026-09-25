use std::time::Duration;

use block_editor_plugin::be_block::AudioContent;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor, FileFilter, PickedFile, content_file_creation};

mod ui;

use ui::AudioView;

const INTRINSIC_SIZE: Vec2 = Vec2::new(320.0, 180.0);

pub struct AudioApp;

impl block_editor_plugin::BeuiApp for AudioApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <AudioView editor={editor} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        content_file_creation::<AudioContent>(&creation, "audio", filter(), decode)
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
    FileFilter::new(
        "Audio",
        "Audio",
        &["mp3", "wav", "ogg", "oga", "flac", "m4a"],
        &["audio/*"],
    )
}

pub(crate) fn guess_media_type(source_name: &str) -> &'static str {
    match source_name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        _ => "audio/mpeg",
    }
}

pub(crate) fn decode(file: PickedFile) -> Result<AudioContent, String> {
    let PickedFile { name, data } = file;
    let media_type = guess_media_type(&name);
    AudioContent::from_file(name.clone(), media_type, data)
        .map_err(|error| format!("Could not import {name}: {error}"))
}
