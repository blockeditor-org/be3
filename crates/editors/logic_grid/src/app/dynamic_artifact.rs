use beui::NodeId;
use beui::reactive::{Frame, List, Show, clone, component, create_memo, view};
use beui::styled::{Caption, Checkbox, use_theme};
use block_editor_beui::be_block::compiled_logic::CompiledLogic;
use block_editor_beui::be_block::{
    ArtifactSource, BlockContent, CompiledLogicContent, CompiledLogicDocument, LogicGridContent,
};
use block_editor_beui::{Artifacts, BlockList, BlockQuery, ContentProjection, EditorHost};
use logicgame::grid::LogicGrid as Grid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ComponentArtifact {
    source: Uuid,
    settings: ComponentSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ComponentSettings {
    rename_with_source: bool,
}

impl Default for ComponentSettings {
    fn default() -> Self {
        Self {
            rename_with_source: true,
        }
    }
}

impl ComponentArtifact {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("logic grid component descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub(super) fn source(data: &[u8]) -> Result<Uuid, String> {
    ComponentArtifact::decode(data).map(|artifact| artifact.source)
}

pub(super) fn descriptor(source_id: Uuid) -> ArtifactSource {
    ArtifactSource {
        source_type: LogicGridContent::CONTENT_TYPE,
        data: ComponentArtifact {
            source: source_id,
            settings: ComponentSettings::default(),
        }
        .encode(),
    }
}

pub(super) fn generate_initial(source_id: Uuid, grid: &Grid) -> Result<CompiledLogic, String> {
    CompiledLogic::compile(source_id, grid).map_err(|error| error.to_string())
}

pub(super) fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Component")
}

pub(super) fn summary(data: &[u8]) -> String {
    let Ok(artifact) = ComponentArtifact::decode(data) else {
        return "Compiled component".to_owned();
    };
    if artifact.settings.rename_with_source {
        "Compiled component, named after its grid".to_owned()
    } else {
        "Compiled component".to_owned()
    }
}

const SETTINGS_PADDING: f32 = 14.0;
const SETTINGS_SPACING: f32 = 12.0;

#[component]
pub(super) fn Settings(artifacts: Artifacts) -> NodeId {
    let data = artifacts.settings();
    let decoded = create_memo(clone!(data -> move || ComponentArtifact::decode(&data.get()).ok()));
    let readable = create_memo(clone!(decoded -> move || decoded.get().is_some()));
    let unreadable = create_memo(clone!(decoded -> move || decoded.get().is_none()));
    let renamed = create_memo(clone!(decoded -> move || {
        decoded
            .get()
            .is_some_and(|artifact| artifact.settings.rename_with_source)
    }));
    let edited = artifacts.clone();
    let rename = clone!(decoded -> move |on: bool| {
        let Some(mut artifact) = decoded.get_untracked() else {
            return;
        };
        artifact.settings.rename_with_source = on;
        edited.edit_settings(artifact.encode());
    });
    let theme = use_theme();
    view! {
        <Frame
            color={theme.background.clone()}
            padding_horizontal=SETTINGS_PADDING
            padding_vertical=SETTINGS_PADDING
        >
            <List spacing=SETTINGS_SPACING>
                <Show condition={readable}>
                    <Checkbox
                        label="Rename with the grid"
                        checked={renamed}
                        @test_id={"logic-grid.rename-with-source"}
                        on_change={rename}
                    />
                </Show>
                <Show condition={unreadable}>
                    <Caption content="These settings cannot be read." />
                </Show>
            </List>
        </Frame>
    }
}

pub(super) fn regenerate(
    host: &EditorHost,
    target_id: Uuid,
    target_type: Uuid,
    data: &[u8],
) -> Result<CompileRegeneration, String> {
    if target_type != CompiledLogicContent::CONTENT_TYPE {
        return Err(format!(
            "compiling a logic grid expected a Compiled Logic target, found {target_type}"
        ));
    }
    let artifact = ComponentArtifact::decode(data)?;
    Ok(CompileRegeneration {
        host: host.clone(),
        source_id: artifact.source,
        source: host.content_of::<LogicGridContent>(artifact.source),
        source_block: host.blocks().watch(BlockQuery::Block(artifact.source)),
        target: target_id,
        settings: artifact.settings,
    })
}

pub(super) struct CompileRegeneration {
    host: EditorHost,
    source_id: Uuid,
    source: ContentProjection<LogicGridContent>,
    source_block: BlockList,
    target: Uuid,
    settings: ComponentSettings,
}

impl CompileRegeneration {
    pub(super) fn poll(&mut self) -> Option<Result<(), String>> {
        let grid = self.source.read(|content| content.root().grid())?;
        let compiled = match generate_initial(self.source_id, &grid) {
            Ok(compiled) => compiled,
            Err(error) => return Some(Err(error)),
        };
        self.host.replace_content(
            self.target,
            &CompiledLogicContent::new(&CompiledLogicDocument::of(compiled)),
        );
        if self.settings.rename_with_source {
            let source_name = self
                .source_block
                .read()
                .into_iter()
                .next()
                .and_then(|info| info.name)
                .unwrap_or_else(|| "Logic Grid".to_owned());
            self.host
                .blocks()
                .set_name(self.target, Some(artifact_name(&source_name)));
        }
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests;
