use std::any::Any;
use std::cell::{Ref, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use beui_platform_plugin::ModuleRunner;
use block_editor_plugin::headless::HeadlessPlugin as Core;
use block_editor_plugin::{Instance, RegionId};
use block_plugin_api::{EditorInstanceId, ScreenLayout, ScreenPlacement};

use crate::instance::BeuiInstance;
use crate::module::Module;
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
    fn editor(&self) -> Option<Editor>;
}

impl<A: BeuiApp> Parts for BeuiInstance<A> {
    fn editor(&self) -> Option<Editor> {
        BeuiInstance::editor(self)
    }
}

fn parts<A: BeuiApp>(instance: &dyn Instance) -> &dyn Parts {
    (instance as &dyn Any)
        .downcast_ref::<BeuiInstance<A>>()
        .expect("an adopted instance keeps its app")
}

pub struct Drawn {
    pub output: beui::FrameOutput,
    pub windows: Vec<(ScreenPlacement, beui::WindowId)>,
}

pub struct HeadlessPlugin {
    core: Core,
    module: Rc<Module>,
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
        let core = Core::new(id, name, version);
        let module = core.module().local(Module::default);
        Self {
            core,
            module,
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

    pub fn draw(&mut self) -> Option<Drawn> {
        let placements: Vec<ScreenPlacement> = self
            .core
            .draw()
            .into_iter()
            .map(|(placement, _)| placement)
            .collect();
        let output = self
            .core
            .module()
            .with_handler(|handler| ModuleRunner::of(handler)?.take_output())
            .flatten()?;
        let windows = placements
            .into_iter()
            .filter_map(|placement| {
                let window = self.module.window(RegionId {
                    instance: placement.instance,
                    region: placement.region,
                })?;
                Some((placement, window))
            })
            .collect();
        Some(Drawn { output, windows })
    }

    pub fn layout(&self) -> &ScreenLayout {
        self.core.layout()
    }

    pub fn document(&self) -> Option<Ref<'_, beui::Document>> {
        Ref::filter_map(self.module.document.borrow(), Option::as_ref).ok()
    }

    pub fn document_mut(&self) -> Option<RefMut<'_, beui::Document>> {
        RefMut::filter_map(self.module.document.borrow_mut(), Option::as_mut).ok()
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
