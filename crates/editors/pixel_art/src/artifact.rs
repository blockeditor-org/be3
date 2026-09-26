use block_editor_beui::be_block::pixel_art::Artwork;
use block_editor_beui::be_block::{ArtifactSource, BlockContent, ImageContent, PixelArtContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{Frame, List, Show, clone, component, create_memo, view};
use block_editor_beui::beui::styled::{Caption, NumberInput, use_theme};
use block_editor_beui::{ArtifactDescription, Artifacts, EditorHost};
use block_editor_beui::{BlockList, BlockQuery, ContentProjection};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_EXPORT_SCALE: u32 = 16;
const SETTINGS_PADDING: f32 = 14.0;
const SETTINGS_SPACING: f32 = 12.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageArtifact {
    source: Uuid,
    settings: ImageSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ImageSettings {
    scale: u32,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self { scale: 1 }
    }
}

impl ImageArtifact {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("pixel art export descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub fn descriptor(source_id: Uuid) -> ArtifactSource {
    ArtifactSource {
        source_type: PixelArtContent::CONTENT_TYPE,
        data: ImageArtifact {
            source: source_id,
            settings: ImageSettings::default(),
        }
        .encode(),
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
    Ok(ImageContent::from_file(
        format!("{source_name} Export"),
        png,
    ))
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

pub fn describe(data: &[u8]) -> Result<ArtifactDescription, String> {
    let artifact = ImageArtifact::decode(data)?;
    Ok(ArtifactDescription {
        source: artifact.source,
        summary: summary(&artifact.settings),
    })
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
    let decoded = create_memo(clone!(data -> move || ImageArtifact::decode(&data.get()).ok()));
    let readable = create_memo(clone!(decoded -> move || decoded.get().is_some()));
    let scale = create_memo(clone!(decoded -> move || {
        decoded.get().map_or(1.0, |artifact| f64::from(artifact.settings.scale))
    }));
    let caption = create_memo(clone!(decoded -> move || match decoded.get() {
        Some(artifact) => summary(&artifact.settings),
        None => "These settings cannot be read.".to_owned(),
    }));
    let edited = artifacts.clone();
    let scaled = decoded.clone();
    let set_scale = move |value: f64| {
        let Some(mut artifact) = scaled.get_untracked() else {
            return;
        };
        artifact.settings.scale = (value as u32).clamp(1, MAX_EXPORT_SCALE);
        edited.edit_settings(artifact.encode());
    };

    let theme = use_theme();

    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=SETTINGS_PADDING
            padding_vertical=SETTINGS_PADDING
        >
            <List spacing=SETTINGS_SPACING>
                <Show condition={readable}>
                    <NumberInput
                        value={scale}
                        min=1.0
                        max={f64::from(MAX_EXPORT_SCALE)}
                        label="Scale"
                        @test_id={"pixel-art.export-scale"}
                        on_change={set_scale}
                    />
                </Show>
                <Caption content={caption} @test_id={"pixel-art.export-summary"} />
            </List>
        </Frame>
    }
}

pub struct Regeneration {
    host: EditorHost,
    source: ContentProjection<PixelArtContent>,
    named: BlockList,
    target: Uuid,
    settings: ImageSettings,
}

impl Regeneration {
    pub fn start(
        host: &EditorHost,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Self, String> {
        if target_type != ImageContent::CONTENT_TYPE {
            return Err(format!(
                "pixel art export expected an Image target, found {target_type}"
            ));
        }
        let artifact = ImageArtifact::decode(data)?;
        Ok(Self {
            host: host.clone(),
            source: host.content_of::<PixelArtContent>(artifact.source),
            named: host.blocks().watch(BlockQuery::Block(artifact.source)),
            target: target_id,
            settings: artifact.settings,
        })
    }

    pub fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read(|content| content.root().artwork())?;
        let name = self
            .named
            .read()
            .into_iter()
            .next()
            .and_then(|info| info.name)
            .unwrap_or_else(|| "Pixel Art".to_owned());
        let generated = generate(&source, &name, &self.settings);
        Some(generated.map(|image| self.host.replace_content(self.target, &image)))
    }
}

#[cfg(test)]
mod tests;
