use block_editor_beui::be_block::{Artifact, BlockContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{Artifacts, EditorHost};
use block_editor_beui::{BlockList, BlockQuery, Blocks, OwnedContentProjection};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use image_block_types::{ImageContent, ImageFile};
use pixel_art_types::{Artwork, PixelArtContent};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_EXPORT_SCALE: u32 = 16;
const SETTINGS_SPACING: Space = Space::Lg;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageSettings {
    scale: u32,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self { scale: 1 }
    }
}

impl ImageSettings {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("pixel art export descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub fn descriptor(source_id: Uuid) -> Artifact {
    Artifact {
        source: source_id.into(),
        source_type: PixelArtContent::CONTENT_TYPE,
        settings: ImageSettings::default().encode(),
    }
}

pub fn generate_initial(art: &Artwork, source_name: &str) -> Result<ImageContent, String> {
    generate(art, source_name, &ImageSettings::default())
}

fn generate(
    art: &Artwork,
    source_name: &str,
    settings: &ImageSettings,
) -> Result<ImageContent, String> {
    let scale = settings.scale.clamp(1, MAX_EXPORT_SCALE);
    let width = u32::from(art.width()) * scale;
    let height = u32::from(art.height()) * scale;
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            &magnified(art, scale),
            width,
            height,
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(ImageFile::from_file(format!("{source_name} Export"), png))
}

fn magnified(art: &Artwork, scale: u32) -> Vec<u8> {
    let pixels = art.rgba_bytes();
    if scale == 1 {
        return pixels.to_vec();
    }
    let width = usize::from(art.width());
    let scale = scale as usize;
    let mut magnified = Vec::with_capacity(pixels.len() * scale * scale);
    for row in pixels.chunks_exact(width * 4) {
        let start = magnified.len();
        for pixel in row.as_chunks::<4>().0 {
            for _ in 0..scale {
                magnified.extend_from_slice(pixel);
            }
        }
        let magnified_row = magnified[start..].to_vec();
        for _ in 1..scale {
            magnified.extend_from_slice(&magnified_row);
        }
    }
    magnified
}

pub fn describe(data: &[u8]) -> Result<String, String> {
    Ok(summary(&ImageSettings::decode(data)?))
}

fn summary(settings: &ImageSettings) -> String {
    let scale = settings.scale.clamp(1, MAX_EXPORT_SCALE);
    if scale == 1 {
        "PNG export at the original size".to_owned()
    } else {
        format!("PNG export at {scale}x")
    }
}

#[component]
pub fn Settings(artifacts: Artifacts) -> NodeId {
    let data = artifacts.settings();
    let decoded = create_memo(move || ImageSettings::decode(&data.get()).ok());
    let readable = create_memo(move || decoded.get().is_some());
    let scale = create_memo(move || {
        decoded
            .get()
            .map_or(1.0, |settings| f64::from(settings.scale))
    });
    let caption = create_memo(move || match decoded.get() {
        Some(settings) => summary(&settings),
        None => "These settings cannot be read.".to_owned(),
    });
    let edited = artifacts.clone();
    let set_scale = move |value: f64| {
        let Some(mut settings) = decoded.get_untracked() else {
            return;
        };
        settings.scale = (value as u32).clamp(1, MAX_EXPORT_SCALE);
        edited.edit_settings(settings.encode());
    };

    view! {
        <List spacing=SETTINGS_SPACING>
            <Show condition={readable}>
                <NumberInput
                    value={Binding::new(scale, set_scale.clone())}
                    min=1.0
                    max={f64::from(MAX_EXPORT_SCALE)}
                    label="Scale"
                    @test_id={"pixel-art.export-scale"}
                />
            </Show>
            <Text style=TextStyle::CAPTION @test_id={"pixel-art.export-summary"}>{caption}</Text>
        </List>
    }
}

pub struct Regeneration {
    host: EditorHost,
    source: OwnedContentProjection<PixelArtContent>,
    named: BlockList,
    target: Uuid,
    settings: ImageSettings,
}

impl Regeneration {
    pub fn start(
        host: &EditorHost,
        target_id: Uuid,
        target_type: Uuid,
        source: Uuid,
        data: &[u8],
    ) -> Result<Self, String> {
        if target_type != ImageContent::CONTENT_TYPE {
            return Err(format!(
                "pixel art export expected an Image target, found {target_type}"
            ));
        }
        let settings = ImageSettings::decode(data)?;
        Ok(Self {
            host: host.clone(),
            source: host.content_of::<PixelArtContent>(source).into(),
            named: Blocks::of(host).watch(BlockQuery::Block(source)),
            target: target_id,
            settings,
        })
    }

    pub fn poll(&mut self) -> Option<Result<(), String>> {
        let source = match self.source.read(|content| content.root().artwork()) {
            Ok(source) => source,
            Err(Pending::Loading) => return None,
            Err(Pending::Failed(error)) => return Some(Err(error)),
        };
        let name = self
            .named
            .read()
            .ok()
            .and_then(|blocks| blocks.into_iter().next())
            .and_then(|info| info.name())
            .unwrap_or_else(|| "Pixel Art".to_owned());
        let generated = generate(&source, &name, &self.settings);
        Some(generated.map(|image| self.host.replace_content(self.target, &image)))
    }
}

#[cfg(test)]
mod tests;
