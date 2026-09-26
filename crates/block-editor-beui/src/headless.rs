use std::any::Any;
use std::collections::HashMap;

use block_editor_plugin::Instance;
use block_editor_plugin::headless::HeadlessPlugin as Core;
use block_plugin_api::{EditorInstanceId, EditorRegion, Message, ScreenLayout, ScreenPlacement};

use crate::beui_frame::BeuiFrame;
use crate::instance::BeuiInstance;
use crate::{Artifacts, BeuiApp, Creation, Editor, EditorHost};

pub type View = Box<dyn FnOnce() -> beui::NodeId>;

pub enum Adopted {
    Editor(Editor, Option<View>),
    Preview(Editor),
    Creation(Creation),
    Artifacts(Artifacts),
}

impl Adopted {
    pub(crate) fn host(&self) -> EditorHost {
        match self {
            Self::Editor(editor, _) | Self::Preview(editor) => editor.host().clone(),
            Self::Creation(creation) => creation.host().clone(),
            Self::Artifacts(artifacts) => artifacts.host().clone(),
        }
    }
}

pub(crate) trait Parts {
    fn document(&self, region: EditorRegion) -> Option<&beui::Document>;
    fn editor(&self) -> Option<Editor>;
    fn chrome_mut(&mut self) -> Option<&mut BeuiFrame>;
    fn take_output(&mut self, region: EditorRegion) -> Option<beui::FrameOutput>;
}

impl<A: BeuiApp> Parts for BeuiInstance<A> {
    fn document(&self, region: EditorRegion) -> Option<&beui::Document> {
        BeuiInstance::document(self, region)
    }

    fn editor(&self) -> Option<Editor> {
        BeuiInstance::editor(self)
    }

    fn chrome_mut(&mut self) -> Option<&mut BeuiFrame> {
        BeuiInstance::chrome_mut(self)
    }

    fn take_output(&mut self, region: EditorRegion) -> Option<beui::FrameOutput> {
        BeuiInstance::take_output(self, region)
    }
}

struct Access {
    shared: fn(&dyn Instance) -> &dyn Parts,
    unique: fn(&mut dyn Instance) -> &mut dyn Parts,
}

fn shared<A: BeuiApp>(instance: &dyn Instance) -> &dyn Parts {
    (instance as &dyn Any)
        .downcast_ref::<BeuiInstance<A>>()
        .expect("an adopted instance keeps its app")
}

fn unique<A: BeuiApp>(instance: &mut dyn Instance) -> &mut dyn Parts {
    (instance as &mut dyn Any)
        .downcast_mut::<BeuiInstance<A>>()
        .expect("an adopted instance keeps its app")
}

pub struct HeadlessPlugin {
    core: Core,
    access: HashMap<EditorInstanceId, Access>,
}

impl HeadlessPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        Self {
            core: Core::new(id, name, version),
            access: HashMap::new(),
        }
    }

    pub fn hello(&self) -> Message {
        self.core.hello()
    }

    pub fn adopt<A: BeuiApp>(&mut self, instance: EditorInstanceId, adopted: Adopted) {
        let host = adopted.host();
        self.core.adopt(
            instance,
            Box::new(BeuiInstance::<A>::adopting(adopted)),
            host,
        );
        self.access.insert(
            instance,
            Access {
                shared: shared::<A>,
                unique: unique::<A>,
            },
        );
    }

    pub fn receive(&mut self, message: Message) -> Vec<Message> {
        self.core.receive(message)
    }

    pub fn draw(&mut self) -> Vec<(ScreenPlacement, beui::FrameOutput)> {
        let placements: Vec<ScreenPlacement> = self
            .core
            .draw()
            .into_iter()
            .map(|(placement, _)| placement)
            .collect();
        placements
            .into_iter()
            .filter_map(|placement| {
                let output = self
                    .parts_mut(placement.instance)?
                    .take_output(placement.region)?;
                Some((placement, output))
            })
            .collect()
    }

    pub fn outbound(&mut self) -> Vec<Message> {
        self.core.outbound()
    }

    pub fn layout(&self) -> &ScreenLayout {
        self.core.layout()
    }

    pub fn document(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<&beui::Document> {
        self.parts(instance)?.document(region)
    }

    pub fn host(&self, instance: EditorInstanceId) -> Option<EditorHost> {
        self.core.host(instance)
    }

    pub fn editor(&self, instance: EditorInstanceId) -> Option<Editor> {
        self.parts(instance)?.editor()
    }

    pub fn in_frame<T>(
        &mut self,
        instance: EditorInstanceId,
        run: impl FnOnce() -> T,
    ) -> Option<T> {
        let chrome = self.parts_mut(instance)?.chrome_mut()?;
        Some(beui::reactive::with_reactive_scope(
            chrome.document_mut(),
            run,
        ))
    }

    fn parts(&self, instance: EditorInstanceId) -> Option<&dyn Parts> {
        let access = self.access.get(&instance)?;
        Some((access.shared)(self.core.instance(instance)?))
    }

    fn parts_mut(&mut self, instance: EditorInstanceId) -> Option<&mut dyn Parts> {
        let unique = self.access.get(&instance)?.unique;
        Some(unique(self.core.instance_mut(instance)?))
    }
}
