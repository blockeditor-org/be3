use std::any::Any;
use std::cell::{Ref, RefMut};
use std::collections::HashMap;

use beui_platform_plugin::RegionRunner;
use block_editor_plugin::Instance;
use block_editor_plugin::headless::HeadlessPlugin as Core;
use block_plugin_api::{EditorInstanceId, EditorRegion, ScreenLayout, ScreenPlacement};

use crate::instance::BeuiInstance;
use crate::{Artifacts, BeuiApp, Creation, Editor, EditorHost, OwnedEditor};

pub type View = Box<dyn FnOnce() -> beui::styled::Root>;

pub enum Adopted {
    Editor(OwnedEditor, Option<View>),
    Preview(OwnedEditor),
    Creation(Creation),
    Artifacts(Artifacts),
}

impl Adopted {
    pub(crate) fn host(&self) -> EditorHost {
        match self {
            Self::Editor(editor, _) | Self::Preview(editor) => editor.host(),
            Self::Creation(creation) => creation.host().clone(),
            Self::Artifacts(artifacts) => artifacts.host().clone(),
        }
    }
}

pub(crate) trait Parts {
    fn document(&self, region: EditorRegion) -> Option<Ref<'_, beui::Document>>;
    fn document_mut(&self, region: EditorRegion) -> Option<RefMut<'_, beui::Document>>;
    fn editor(&self) -> Option<Editor>;
}

impl<A: BeuiApp> Parts for BeuiInstance<A> {
    fn document(&self, region: EditorRegion) -> Option<Ref<'_, beui::Document>> {
        BeuiInstance::document(self, region)
    }

    fn document_mut(&self, region: EditorRegion) -> Option<RefMut<'_, beui::Document>> {
        BeuiInstance::document_mut(self, region)
    }

    fn editor(&self) -> Option<Editor> {
        BeuiInstance::editor(self)
    }
}

fn parts<A: BeuiApp>(instance: &dyn Instance) -> &dyn Parts {
    (instance as &dyn Any)
        .downcast_ref::<BeuiInstance<A>>()
        .expect("an adopted instance keeps its app")
}

pub struct HeadlessPlugin {
    core: Core,
    access: HashMap<EditorInstanceId, fn(&dyn Instance) -> &dyn Parts>,
}

impl std::ops::Deref for HeadlessPlugin {
    type Target = Core;

    fn deref(&self) -> &Core {
        &self.core
    }
}

impl std::ops::DerefMut for HeadlessPlugin {
    fn deref_mut(&mut self) -> &mut Core {
        &mut self.core
    }
}

impl HeadlessPlugin {
    pub fn new(id: &str, name: &str, version: &str) -> Self {
        crate::fonts::use_fonts(beui::FontLibrary::bundled());
        Self {
            core: Core::new(id, name, version),
            access: HashMap::new(),
        }
    }

    pub fn adopt<A: BeuiApp>(&mut self, instance: EditorInstanceId, adopted: Adopted) {
        let host = adopted.host();
        self.core.adopt(
            instance,
            Box::new(BeuiInstance::<A>::adopting(adopted)),
            host,
        );
        self.access.insert(instance, parts::<A>);
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
                let handler = self
                    .core
                    .region_handler_mut(placement.instance, placement.region)?;
                let output = RegionRunner::of(handler)?.take_output()?;
                Some((placement, output))
            })
            .collect()
    }

    pub fn layout(&self) -> &ScreenLayout {
        self.core.layout()
    }

    pub fn document(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<Ref<'_, beui::Document>> {
        self.parts(instance)?.document(region)
    }

    pub fn document_mut(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<RefMut<'_, beui::Document>> {
        self.parts(instance)?.document_mut(region)
    }

    pub fn host(&self, instance: EditorInstanceId) -> Option<EditorHost> {
        self.core.host(instance)
    }

    pub fn editor(&self, instance: EditorInstanceId) -> Option<Editor> {
        self.parts(instance)?.editor()
    }

    fn parts(&self, instance: EditorInstanceId) -> Option<&dyn Parts> {
        let access = self.access.get(&instance)?;
        Some(access(self.core.instance(instance)?))
    }
}
