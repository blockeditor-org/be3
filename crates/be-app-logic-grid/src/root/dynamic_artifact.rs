use be_sdk_beui::be_block_content::{Artifact, BlockContent};
use be_sdk_beui::{BlockQuery, Blocks, Editor, ExportSettings, ExporterOf, Made};
use be_type_logic_grid::CompiledLogic;
use be_type_logic_grid::{CompiledLogicContent, CompiledLogicDocument, LogicGridContent};
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use logicgame::grid::LogicGrid as Grid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default)]
pub(super) struct ComponentSettings {
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
    #[cfg(test)]
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
        exporter: EXPORTER.to_owned(),
        settings: ComponentSettings::default().encode(),
        made: None,
    }
}

/// The name the manifest declares the grid's one exporter under.
const EXPORTER: &str = "component";

/// The grid compiled into a component (guides/apps/editor.md, "Exporting"), which the grid's
/// Compile makes in the workspace and the Export dialog makes anywhere.
pub(super) fn exporter() -> ExporterOf<CompiledLogicContent, ComponentSettings> {
    ExporterOf::new(EXPORTER, |editor: Editor| {
        let source = editor.block_id();
        let content = editor.block_content::<LogicGridContent>();
        let listed = Blocks::of(&editor.host()).watch(BlockQuery::Block(source));
        move |settings: &ComponentSettings| {
            let grid = content.read(|content| content.root().grid())?;
            let compiled = generate_initial(source, &grid).map_err(Pending::Failed)?;
            let source_name = listed
                .read()?
                .into_iter()
                .next()
                .and_then(|info| info.name())
                .unwrap_or_else(|| "Logic Grid".to_owned());
            Ok(Made {
                content: CompiledLogicContent::new(&CompiledLogicDocument::of(compiled)),
                name: artifact_name(&source_name),
                renames: settings.rename_with_source,
            })
        }
    })
    .settings(|settings| view! { <Settings settings /> })
    .describe(|settings: &ComponentSettings| summary_of(settings).to_owned())
}

pub(super) fn generate_initial(source_id: Uuid, grid: &Grid) -> Result<CompiledLogic, String> {
    CompiledLogic::compile(source_id, grid).map_err(|error| error.to_string())
}

pub(super) fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Component")
}

#[cfg(test)]
pub(super) fn summary(data: &[u8]) -> String {
    ComponentSettings::decode(data)
        .map_or("Compiled component", |settings| summary_of(&settings))
        .to_owned()
}

fn summary_of(settings: &ComponentSettings) -> &'static str {
    match settings.rename_with_source {
        true => "Compiled component, named after its grid",
        false => "Compiled component",
    }
}

const SETTINGS_SPACING: Space = Space::Lg;

#[component]
fn Settings(settings: ExportSettings<ComponentSettings>) -> NodeId {
    let value = settings.value();
    let renamed = create_memo(move || value.get().rename_with_source);
    let rename = move |on: bool| settings.update(|settings| settings.rename_with_source = on);
    view! {
        <List spacing=SETTINGS_SPACING>
            <Checkbox
                label="Rename with the grid"
                checked={Binding::new(renamed, rename)}
                @test_id={"logic-grid.rename-with-source"}
            />
        </List>
    }
}

#[cfg(test)]
mod tests;
