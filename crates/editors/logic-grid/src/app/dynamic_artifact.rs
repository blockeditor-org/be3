use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_editor_beui::be_block::{Artifact, BlockContent};
use block_editor_beui::{Artifacts, BlockList, BlockQuery, EditorHost, OwnedContentProjection};
use logic_grid_types::CompiledLogic;
use logic_grid_types::{CompiledLogicContent, CompiledLogicDocument, LogicGridContent};
use logicgame::grid::LogicGrid as Grid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl ComponentSettings {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("logic grid component descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub(super) fn descriptor(source_id: Uuid) -> Artifact {
    Artifact {
        source: source_id.into(),
        source_type: LogicGridContent::CONTENT_TYPE,
        settings: ComponentSettings::default().encode(),
    }
}

pub(super) fn generate_initial(source_id: Uuid, grid: &Grid) -> Result<CompiledLogic, String> {
    CompiledLogic::compile(source_id, grid).map_err(|error| error.to_string())
}

pub(super) fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Component")
}

pub(super) fn summary(data: &[u8]) -> String {
    let Ok(settings) = ComponentSettings::decode(data) else {
        return "Compiled component".to_owned();
    };
    if settings.rename_with_source {
        "Compiled component, named after its grid".to_owned()
    } else {
        "Compiled component".to_owned()
    }
}

const SETTINGS_SPACING: Space = Space::Lg;

#[component]
pub(super) fn Settings(artifacts: Artifacts) -> NodeId {
    let data = artifacts.settings();
    let decoded = create_memo(move || ComponentSettings::decode(&data.get()).ok());
    let readable = create_memo(move || decoded.get().is_some());
    let unreadable = create_memo(move || decoded.get().is_none());
    let renamed = create_memo(move || {
        decoded
            .get()
            .is_some_and(|settings| settings.rename_with_source)
    });
    let edited = artifacts.clone();
    let rename = move |on: bool| {
        let Some(mut settings) = decoded.get_untracked() else {
            return;
        };
        settings.rename_with_source = on;
        edited.edit_settings(settings.encode());
    };
    view! {
        <List spacing=SETTINGS_SPACING>
            <Show condition={readable}>
                <Checkbox
                    label="Rename with the grid"
                    checked={Binding::new(renamed, rename.clone())}
                    @test_id={"logic-grid.rename-with-source"}
                />
            </Show>
            <Show condition={unreadable}>
                <Text style=TextStyle::CAPTION>"These settings cannot be read."</Text>
            </Show>
        </List>
    }
}

pub(super) fn regenerate(
    host: &EditorHost,
    target_id: Uuid,
    target_type: Uuid,
    source: Uuid,
    data: &[u8],
) -> Result<CompileRegeneration, String> {
    if target_type != CompiledLogicContent::CONTENT_TYPE {
        return Err(format!(
            "compiling a logic grid expected a Compiled Logic target, found {target_type}"
        ));
    }
    let settings = ComponentSettings::decode(data)?;
    Ok(CompileRegeneration {
        host: host.clone(),
        source_id: source,
        source: host.content_of::<LogicGridContent>(source),
        source_block: host.blocks().watch(BlockQuery::Block(source)),
        target: target_id,
        settings,
    })
}

pub(super) struct CompileRegeneration {
    host: EditorHost,
    source_id: Uuid,
    source: OwnedContentProjection<LogicGridContent>,
    source_block: BlockList,
    target: Uuid,
    settings: ComponentSettings,
}

impl CompileRegeneration {
    pub(super) fn poll(&mut self) -> Option<Result<(), String>> {
        let grid = match self.source.read(|content| content.root().grid()) {
            Ok(grid) => grid,
            Err(Pending::Loading) => return None,
            Err(Pending::Failed(error)) => return Some(Err(error)),
        };
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
                .ok()
                .and_then(|listed| listed.into_iter().next())
                .and_then(|info| info.name())
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
