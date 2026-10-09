use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use beui_adapter_plugin::PluginSurface;
use block_editor_plugin::{
    Artifact, ArtifactDescription, EditorHost, EditorRegion, Frame, Instance, Region,
};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceRect};
use block_plugin_api::InputEvent;
use uuid::Uuid;

use crate::beui_frame::{self, BeuiFrame, FrameBar};
use crate::editor::BeuiScale;
use crate::{Artifacts, BeuiApp, Creation, Editor};

pub struct BeuiPlugin<A: BeuiApp>(PhantomData<A>);

impl<A: BeuiApp> block_editor_plugin::Plugin for BeuiPlugin<A> {
    fn open(host: EditorHost) -> Box<dyn Instance> {
        Box::new(BeuiInstance::<A>::new(host))
    }
}

pub(crate) struct BeuiInstance<A: BeuiApp> {
    host: EditorHost,
    scale: Rc<Cell<BeuiScale>>,
    surfaces: HashMap<EditorRegion, PluginSurface>,
    views: Views<A>,
    lent: Rc<RefCell<Option<Lent<A>>>>,
}

struct Lent<A: BeuiApp> {
    views: Views<A>,
    region: Region,
    picks: Vec<String>,
    settings: Option<Vec<u8>>,
    shown: Shown,
}

#[derive(Default)]
struct Shown {
    content: Option<beui::Rect>,
    painted: Vec<beui::Rect>,
    floating: Vec<beui::Rect>,
    claims: Vec<(beui::Modifiers, beui::Rect)>,
    exit: bool,
}

struct RegionApp<A: BeuiApp> {
    region: EditorRegion,
    lent: Rc<RefCell<Option<Lent<A>>>>,
}

impl<A: BeuiApp> beui::App for RegionApp<A> {
    fn update(&mut self, context: &beui::Context, rect: beui::Rect) {
        let mut lent = self.lent.borrow_mut();
        let Some(lent) = lent.as_mut() else {
            return;
        };
        let Lent {
            views,
            region,
            picks,
            settings,
            shown,
        } = lent;
        match self.region {
            EditorRegion::Frame if views.creating => shown.content = views.creation(context, rect),
            EditorRegion::Frame => views.frame(context, rect, region, picks, shown),
            EditorRegion::Preview => views.preview(context, rect),
            EditorRegion::ArtifactSettings => {
                if let Some(draft) = settings {
                    views.artifact_settings(context, rect, draft);
                }
            }
        }
    }

    fn clear_color(&self) -> beui::Color32 {
        beui::Color32::TRANSPARENT
    }
}

struct Views<A: BeuiApp> {
    creating: bool,
    chrome: Option<BeuiFrame>,
    editor: Option<Editor>,
    preview: Option<Editor>,
    preview_document: Option<beui::Document>,
    creation: Option<Creation>,
    dialog: Option<beui::Document>,
    artifacts: Option<Artifacts>,
    settings: Option<beui::Document>,
    view: Option<crate::headless::View>,
    app: PhantomData<A>,
}

impl<A: BeuiApp> Views<A> {
    fn preview(&mut self, context: &beui::Context, rect: beui::Rect) {
        let Some(editor) = self.preview.clone() else {
            return;
        };
        let document = self.preview_document.get_or_insert_with(|| {
            let built = editor.clone();
            beui::reactive::build(move || A::preview_view(built))
        });
        let begun = editor.clone();
        beui::reactive::with_reactive_scope(document, move || begun.begin_frame());
        document.show(context, rect);
        editor.end_frame(document);
    }

    fn creation(&mut self, context: &beui::Context, rect: beui::Rect) -> Option<beui::Rect> {
        let (Some(creation), Some(dialog)) = (self.creation.as_ref(), self.dialog.as_mut()) else {
            return None;
        };
        let creation = creation.clone();
        beui::reactive::with_reactive_scope(dialog, move || creation.begin_frame());
        dialog.show(context, rect);
        let measured = dialog.measure_root(context, beui::vec2(rect.width(), f32::INFINITY))?;
        measured
            .y
            .is_finite()
            .then(|| beui::Rect::from_min_size(rect.min, beui::vec2(rect.width(), measured.y)))
    }

    fn artifact_settings(
        &mut self,
        context: &beui::Context,
        rect: beui::Rect,
        draft: &mut Vec<u8>,
    ) {
        let Some(artifacts) = self.artifacts.clone() else {
            return;
        };
        let document = self.settings.get_or_insert_with(|| {
            let built = artifacts.clone();
            beui::reactive::build(move || A::artifact_settings_view(built))
        });
        let received = artifacts.clone();
        let data = std::mem::take(draft);
        beui::reactive::with_reactive_scope(document, move || received.receive_settings(&data));
        document.show(context, rect);
        *draft = artifacts
            .take_settings_edit()
            .unwrap_or_else(|| artifacts.settings().get_untracked());
    }
}

impl<A: BeuiApp> Views<A> {
    fn empty() -> Self {
        Self {
            creating: false,
            chrome: None,
            editor: None,
            preview: None,
            preview_document: None,
            creation: None,
            dialog: None,
            artifacts: None,
            settings: None,
            view: None,
            app: PhantomData,
        }
    }

    fn frame(
        &mut self,
        context: &beui::Context,
        rect: beui::Rect,
        region: &Region,
        picks: &[String],
        shown: &mut Shown,
    ) {
        let spec = &region.spec;
        let chrome = self
            .chrome
            .as_mut()
            .expect("the frame chrome is built before its region runs");
        let set_bar = chrome.set_bar();
        let bar = FrameBar {
            shown: region.chrome_drawn() && (spec.top_bar.shown() || spec.content.is_some()),
            closable: spec.content.is_some(),
            on_phone: spec.top_bar.phone(),
        };
        let menu = chrome.menu();
        let editor = self.editor.clone();
        beui::reactive::with_reactive_scope(chrome.document_mut(), || {
            set_bar.set(bar);
            for id in picks {
                beui_frame::run_menu_pick(&menu, id);
            }
            if let Some(editor) = &editor {
                editor.begin_frame();
            }
        });
        chrome.document_mut().show(context, rect);
        if let Some(editor) = &editor {
            editor.end_frame(chrome.document());
        }
        shown.content = chrome.document().node_rect(chrome.content());
        shown.exit = chrome.exit().get() || beui_frame::escaped(context);
        shown.painted = vec![rect];
        shown.floating = chrome.document().overlay_rects();
        shown.claims = chrome.document().press_claims();
    }

    fn build_chrome(&mut self) {
        if self.creating || self.chrome.is_some() {
            return;
        }
        let editor = self
            .editor
            .clone()
            .expect("connect is called before the view is built");
        let built = editor.clone();
        let view = self.view.take();
        self.chrome = Some(BeuiFrame::build(&editor, move || match view {
            Some(view) => view(),
            None => A::view(built),
        }));
    }
}

impl<A: BeuiApp> BeuiInstance<A> {
    pub(crate) fn adopting(adopted: crate::headless::Adopted) -> Self {
        let mut instance = Self::new(adopted.host());
        let views = &mut instance.views;
        match adopted {
            crate::headless::Adopted::Editor(editor, view) => {
                views.editor = Some(editor);
                views.view = view;
            }
            crate::headless::Adopted::Preview(editor) => views.preview = Some(editor),
            crate::headless::Adopted::Creation(creation) => views.creation = Some(creation),
            crate::headless::Adopted::Artifacts(artifacts) => views.artifacts = Some(artifacts),
        }
        instance
    }

    pub(crate) fn editor(&self) -> Option<Editor> {
        self.views.editor.clone()
    }

    pub(crate) fn document(&self, region: EditorRegion) -> Option<&beui::Document> {
        match (region, self.views.creating) {
            (EditorRegion::Frame, false) => self.views.chrome.as_ref().map(BeuiFrame::document),
            (EditorRegion::Frame, true) => self.views.dialog.as_ref(),
            (EditorRegion::Preview, _) => self.views.preview_document.as_ref(),
            (EditorRegion::ArtifactSettings, _) => self.views.settings.as_ref(),
        }
    }

    pub(crate) fn chrome_mut(&mut self) -> Option<&mut BeuiFrame> {
        self.views.chrome.as_mut()
    }

    pub(crate) fn take_output(&mut self, region: EditorRegion) -> Option<beui::FrameOutput> {
        self.surfaces.get_mut(&region)?.take_output()
    }

    fn new(host: EditorHost) -> Self {
        Self {
            host,
            scale: Rc::default(),
            surfaces: HashMap::new(),
            views: Views::empty(),
            lent: Rc::default(),
        }
    }

    fn surface(&mut self, region: EditorRegion) -> &mut PluginSurface {
        let host = &self.host;
        let lent = &self.lent;
        self.surfaces.entry(region).or_insert_with(|| {
            PluginSurface::launch(
                host.clone(),
                crate::fonts::context(),
                RegionApp::<A> {
                    region,
                    lent: Rc::clone(lent),
                },
            )
        })
    }

    fn zones_pending(&self) -> bool {
        let frame = self
            .views
            .chrome
            .as_ref()
            .map(|chrome| chrome.document().zone());
        frame.into_iter().any(beui::reactive::zone_pending)
    }
}

impl<A: BeuiApp> Instance for BeuiInstance<A> {
    fn connect(&mut self, block_id: Uuid) {
        let scaled = |host: EditorHost| Editor::scaled(host, block_id, Rc::clone(&self.scale));
        let fresh = |editor: &Option<Editor>| {
            editor
                .as_ref()
                .is_none_or(|editor| editor.block_id() != block_id)
        };
        if fresh(&self.views.editor) {
            self.views.editor = Some(scaled(self.host.clone()));
            self.views.view = None;
        }
        if fresh(&self.views.preview) {
            self.views.preview = Some(scaled(self.host.clone()));
            self.views.preview_document = None;
        }
    }

    fn connect_creation(&mut self, template: String) {
        self.views.creating = true;
        let creation = self
            .views
            .creation
            .take()
            .unwrap_or_else(|| Creation::for_template(self.host.clone(), template));
        let built = creation.clone();
        self.views.dialog = Some(beui::reactive::build(move || A::creation_view(built)));
        self.views.creation = Some(creation);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .views
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        A::create_block(creation)
    }

    fn connect_artifact(&mut self, artifact: Artifact) {
        let artifacts = match self.views.artifacts.take() {
            Some(artifacts) if artifacts.block_id() == artifact.block_id => artifacts,
            _ => Artifacts::new(self.host.clone(), artifact),
        };
        A::connect_artifact(&artifacts);
        self.views.artifacts = Some(artifacts);
    }

    fn describe_artifact(&mut self, data: &[u8]) -> Result<ArtifactDescription, String> {
        A::describe_artifact(data)
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        if let Some(artifacts) = &self.views.artifacts {
            artifacts.regenerate(data);
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        self.views.artifacts.as_ref()?.poll()
    }

    fn intrinsic_size(&mut self) -> Option<beui::Vec2> {
        self.views
            .editor
            .as_ref()
            .and_then(Editor::intrinsic_size)
            .or_else(|| self.views.preview.as_ref().and_then(Editor::intrinsic_size))
            .or_else(A::intrinsic_size)
    }

    fn resized(&mut self, size: beui::Vec2) {
        if let Some(editor) = &self.views.editor {
            editor.report_resize(size);
        }
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        A::aspect_ratio()
    }

    fn presence_visible(&mut self, visible: bool) {
        if let Some(editor) = &self.views.editor {
            editor.report_presence_visible(visible);
        }
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        let Some(editor) = self.views.editor.clone() else {
            return false;
        };
        let document = self.views.chrome.as_mut().map(BeuiFrame::document_mut);
        match document {
            Some(document) => {
                beui::reactive::with_reactive_scope(document, || editor.replace_child(old, new))
            }
            None => editor.replace_child(old, new),
        }
    }

    fn update(&mut self, region: &Region, settings: Option<&mut Vec<u8>>) -> Frame {
        let picks = match region.region {
            EditorRegion::Frame => self.host.take_menu_picks(),
            _ => Vec::new(),
        };
        if region.region == EditorRegion::Frame {
            self.views.build_chrome();
        }
        let scale = Rc::clone(&self.scale);
        let surface = self.surface(region.region);
        let ratio = surface.ratio(region);
        scale.set(BeuiScale {
            ratio,
            pixels_per_point: surface.context().pixels_per_point(),
        });
        let views = std::mem::replace(&mut self.views, Views::empty());
        let mut draft = settings;
        *self.lent.borrow_mut() = Some(Lent {
            views,
            region: region.clone(),
            picks,
            settings: draft.as_deref_mut().map(std::mem::take),
            shown: Shown::default(),
        });
        let surface = self
            .surfaces
            .get_mut(&region.region)
            .expect("the region's surface was just launched");
        let mut frame = surface.update(region);
        let lent = self
            .lent
            .borrow_mut()
            .take()
            .expect("the region gives back what it was lent");
        self.views = lent.views;
        if let (Some(draft), Some(edited)) = (draft, lent.settings) {
            *draft = edited;
        }
        let shown = lent.shown;
        if shown.exit {
            self.host.leave_frame();
        }
        let to_region = |rect: beui::Rect| surface.to_region(region, rect);
        frame.content = shown.content.map(to_region);
        frame.painted = shown.painted.into_iter().map(to_region).collect();
        frame.floating = shown.floating.into_iter().map(to_region).collect();
        frame.claims = shown
            .claims
            .into_iter()
            .map(|(held, rect)| block_editor_plugin::Claim {
                modifiers: block_plugin_api::Modifiers {
                    alt: held.alt,
                    control: held.ctrl,
                    shift: held.shift,
                    logo: held.logo,
                },
                rect: to_region(rect),
            })
            .collect();
        let locked = self.surfaces.values().any(PluginSurface::pointer_locked);
        self.host.grab_cursor(locked);
        if self.zones_pending() {
            frame.repaint_after = Some(Duration::ZERO);
        }
        frame
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        match self.surfaces.get_mut(&target.region) {
            Some(surface) => surface.paint(target),
            None => Vec::new(),
        }
    }

    fn input(&mut self, region: &Region, event: &InputEvent) {
        self.surface(region.region).input(region, event);
    }
}

#[cfg(test)]
mod tests;
