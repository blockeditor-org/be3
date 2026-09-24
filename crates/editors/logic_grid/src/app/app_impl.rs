use std::cell::RefCell;

use beui::NodeId;
use beui::reactive::view;
use block_editor_plugin::{ArtifactDescription, Artifacts, Creation, Editor};

use super::ui::LogicGridView;
use super::*;

pub struct LogicGridApp;

impl block_editor_plugin::BeuiApp for LogicGridApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <LogicGridView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        let block = creation.client().create_block(LogicGrid::new());
        creation.seed_content(block.id(), &LogicGridContent::default());
        Ok(block.id())
    }

    fn connect_artifact(artifacts: &Artifacts) {
        let regeneration: Rc<RefCell<Option<dynamic_artifact::CompileRegeneration>>> =
            Rc::new(RefCell::default());
        let failure: Rc<RefCell<Option<String>>> = Rc::new(RefCell::default());
        let client = artifacts.client().clone();
        let host = artifacts.host().clone();
        let block_id = artifacts.block_id();
        let block_type = artifacts.block_type();
        let started = Rc::clone(&regeneration);
        let reported = Rc::clone(&failure);
        artifacts.on_regenerate(move |data| {
            match dynamic_artifact::regenerate(&host, &client, block_id, block_type, data) {
                Ok(started_regeneration) => {
                    *started.borrow_mut() = Some(started_regeneration);
                    reported.borrow_mut().take();
                }
                Err(error) => {
                    started.borrow_mut().take();
                    *reported.borrow_mut() = Some(error);
                }
            }
        });
        artifacts.on_poll(move || {
            if let Some(error) = failure.borrow_mut().take() {
                return Some(Err(error));
            }
            let result = regeneration.borrow_mut().as_mut()?.poll()?;
            regeneration.borrow_mut().take();
            Some(result)
        });
    }

    fn describe_artifact(data: &[u8]) -> Result<ArtifactDescription, String> {
        Ok(ArtifactDescription {
            source: dynamic_artifact::source(data)?,
            summary: dynamic_artifact::summary(data),
        })
    }

    fn artifact_settings_view(artifacts: Artifacts) -> NodeId {
        view! {
            <dynamic_artifact::Settings artifacts={artifacts} />
        }
    }
}
