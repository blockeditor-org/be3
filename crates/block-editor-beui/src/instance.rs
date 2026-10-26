use beui::reactive::Get;
use beui::reactive::Set;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use beui_adapter_plugin::PluginSurface;
use block_editor_plugin::{
    Artifact, ArtifactDescription, EditorHost, EditorRegion, Frame, Instance, Region,
};
#[cfg(target_arch = "wasm32")]
use block_editor_plugin::{PaintTarget, SurfaceRect};
use block_plugin_api::TimedInput;
use uuid::Uuid;

use crate::beui_frame::{self, BeuiFrame, FrameBar};
use crate::editor::BeuiScale;
use crate::{Artifacts, BeuiApp, Creation, Editor, OwnedEditor};

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
    keeps_accessibility: bool,
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
    drawing: Vec<beui::Rect>,
    every_sample: bool,
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
            EditorRegion::Window(_) => {}
        }
    }
}

struct Views<A: BeuiApp> {
    creating: bool,
    chrome: Option<BeuiFrame>,
    preview_document: Option<beui::Document>,
    dialog: Option<beui::Document>,
    settings: Option<beui::Document>,
    view: Option<crate::headless::View>,
    editor: Option<OwnedEditor>,
    preview: Option<OwnedEditor>,
    creation: Option<Creation>,
    artifacts: Option<Artifacts>,
    app: PhantomData<A>,
}

impl<A: BeuiApp> Views<A> {
    fn preview(&mut self, context: &beui::Context, rect: beui::Rect) {
        let Some(editor) = self.preview.as_deref().copied() else {
            return;
        };
        let document = self.preview_document.get_or_insert_with(|| {
            beui::reactive::build(move || {
                beui::reactive::view! {
                    <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                        <A::Preview editor />
                    </beui::styled::Root>
                }
            })
        });
        editor.begin_frame();
        document.show(context, rect);
        editor.end_frame(document);
    }

    fn creation(&mut self, context: &beui::Context, rect: beui::Rect) -> Option<beui::Rect> {
        let (Some(creation), Some(dialog)) = (self.creation.as_ref(), self.dialog.as_mut()) else {
            return None;
        };
        let creation = creation.clone();
        creation.begin_frame();
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
            beui::reactive::build(move || {
                beui::reactive::view! {
                    <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                        <A::ArtifactSettings artifacts={built} />
                    </beui::styled::Root>
                }
            })
        });
        artifacts.receive_settings(&std::mem::take(draft));
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
        let shown_bar = chrome.bar();
        let bar = FrameBar {
            shown: region.chrome_drawn() && (spec.top_bar.shown() || spec.content.is_some()),
            framed: region.chrome_drawn(),
            closable: spec.content.is_some(),
            on_phone: spec.top_bar.phone(),
        };
        let menu = chrome.menu();
        let editor = self.editor.as_deref().copied();
        shown_bar.set(bar);
        if !picks.is_empty() {
            beui::reactive::with_reactive_scope(chrome.document_mut(), || {
                for id in picks {
                    beui_frame::run_menu_pick(&menu, id);
                }
            });
        }
        if let Some(editor) = &editor {
            editor.begin_frame();
        }
        chrome.document_mut().show(context, rect);
        if let Some(editor) = &editor {
            editor.end_frame(chrome.document());
        }
        shown.content = chrome.document().node_rect(chrome.content());
        shown.exit = chrome.exit().take();
        shown.painted = vec![rect];
        shown.floating = chrome.document().overlay_rects();
        shown.drawing = chrome.document().drawing_rects();
        shown.every_sample = chrome.document().wants_every_sample();
        shown.claims = chrome.document().press_claims();
    }

    fn build_chrome(&mut self) {
        if self.creating || self.chrome.is_some() {
            return;
        }
        let editor = self
            .editor
            .as_deref()
            .copied()
            .expect("connect is called before the view is built");
        let view = self.view.take();
        self.chrome = Some(BeuiFrame::build(editor, move || match view {
            Some(view) => view(),
            None => beui::reactive::view! {
                <A::View editor />
            },
        }));
    }
}

impl<A: BeuiApp> BeuiInstance<A> {
    pub(crate) fn adopting(adopted: crate::headless::Adopted) -> Self {
        let mut instance = Self::new(adopted.host());
        instance.keeps_accessibility = true;
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
        self.views.editor.as_deref().copied()
    }

    pub(crate) fn document(&self, region: EditorRegion) -> Option<&beui::Document> {
        match (region, self.views.creating) {
            (EditorRegion::Frame | EditorRegion::Window(_), false) => self.views.chrome.as_ref().map(BeuiFrame::document),
            (EditorRegion::Frame | EditorRegion::Window(_), true) => self.views.dialog.as_ref(),
            (EditorRegion::Preview, _) => self.views.preview_document.as_ref(),
            (EditorRegion::ArtifactSettings, _) => self.views.settings.as_ref(),
        }
    }

    pub(crate) fn document_mut(&mut self, region: EditorRegion) -> Option<&mut beui::Document> {
        match (region, self.views.creating) {
            (EditorRegion::Frame | EditorRegion::Window(_), false) => self.views.chrome.as_mut().map(BeuiFrame::document_mut),
            (EditorRegion::Frame | EditorRegion::Window(_), true) => self.views.dialog.as_mut(),
            (EditorRegion::Preview, _) => self.views.preview_document.as_mut(),
            (EditorRegion::ArtifactSettings, _) => self.views.settings.as_mut(),
        }
    }

    pub(crate) fn take_output(&mut self, region: EditorRegion) -> Option<beui::FrameOutput> {
        self.surfaces.get_mut(&region)?.take_output()
    }

    fn framing(&mut self) -> Option<&mut PluginSurface> {
        self.surfaces.get_mut(&EditorRegion::Frame)
    }

    fn new(host: EditorHost) -> Self {
        Self {
            host,
            scale: Rc::default(),
            surfaces: HashMap::new(),
            views: Views::empty(),
            lent: Rc::default(),
            keeps_accessibility: false,
        }
    }

    fn surface(&mut self, region: EditorRegion) -> &mut PluginSurface {
        let host = &self.host;
        let lent = &self.lent;
        let keeps_accessibility = self.keeps_accessibility;
        self.surfaces.entry(region).or_insert_with(|| {
            let surface = PluginSurface::launch(
                host.clone(),
                crate::fonts::context(),
                RegionApp::<A> {
                    region,
                    lent: Rc::clone(lent),
                },
            );
            surface.context().keep_accessibility(keeps_accessibility);
            surface
        })
    }
}

impl<A: BeuiApp> Instance for BeuiInstance<A> {
    fn connect(&mut self, block_id: Uuid) {
        let fresh = |editor: &Option<OwnedEditor>| {
            editor
                .as_ref()
                .is_none_or(|editor| editor.block_id() != block_id)
        };
        let contents = [&self.views.editor, &self.views.preview]
            .into_iter()
            .flatten()
            .find(|editor| editor.block_id() == block_id)
            .map(OwnedEditor::contents)
            .unwrap_or_default();
        let scaled = |host: EditorHost| {
            OwnedEditor::scaled(host, block_id, Rc::clone(&self.scale), Rc::clone(&contents))
        };
        if fresh(&self.views.editor) {
            self.views.chrome = None;
            self.views.editor = Some(scaled(self.host.clone()));
            self.views.view = None;
        }
        if fresh(&self.views.preview) {
            self.views.preview_document = None;
            self.views.preview = Some(scaled(self.host.clone()));
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
        self.views.dialog = Some(beui::reactive::build(move || {
            beui::reactive::view! {
                <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                    <A::Creation creation={built} />
                </beui::styled::Root>
            }
        }));
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
            .as_deref()
            .and_then(Editor::intrinsic_size)
            .or_else(|| {
                self.views
                    .preview
                    .as_deref()
                    .and_then(Editor::intrinsic_size)
            })
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
        let Some(editor) = self.views.editor.as_deref().copied() else {
            return false;
        };
        editor.replace_child(old, new)
    }

    fn window_reported(&mut self, report: block_plugin_api::WindowReport) {
        if let Some(surface) = self.framing() {
            surface.window_reported(report);
        }
    }

    fn update(&mut self, region: &Region, settings: Option<&mut Vec<u8>>) -> Frame {
        if let EditorRegion::Window(window) = region.region {
            return self
                .framing()
                .map(|surface| surface.update_window(window, region))
                .unwrap_or_default();
        }
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
        frame.drawing = shown.drawing.into_iter().map(to_region).collect();
        frame.every_sample = shown.every_sample;
        frame.claims = shown
            .claims
            .into_iter()
            .map(|(held, rect)| block_editor_plugin::Claim {
                modifiers: held,
                rect: to_region(rect),
            })
            .collect();
        frame
    }

    fn painting(&mut self, region: EditorRegion) -> Option<beui::painting::Painting> {
        match region {
            EditorRegion::Window(window) => self.framing()?.window_painting(window),
            _ => self.surfaces.get(&region)?.painting(),
        }
    }

    fn overflows(&mut self, region: EditorRegion) -> Vec<String> {
        self.surfaces
            .get(&region)
            .map(PluginSurface::overflows)
            .unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    fn paint(&mut self, target: &PaintTarget<'_>) -> Vec<SurfaceRect> {
        if let EditorRegion::Window(window) = target.region {
            return match self.framing() {
                Some(surface) => surface.paint_window(window, target),
                None => Vec::new(),
            };
        }
        match self.surfaces.get_mut(&target.region) {
            Some(surface) => surface.paint(target),
            None => Vec::new(),
        }
    }

    fn input(&mut self, region: &Region, event: &TimedInput) {
        match region.region {
            EditorRegion::Window(window) => {
                if let Some(surface) = self.framing() {
                    surface.window_input(window, region, event);
                }
            }
            _ => self.surface(region.region).input(region, event),
        }
    }

    fn run_action(&mut self, region: &Region, id: &str) {
        let region = match region.region {
            EditorRegion::Window(_) => EditorRegion::Frame,
            region => region,
        };
        self.surface(region).run_action(id);
    }

    fn node_action(
        &mut self,
        region: &Region,
        request: block_editor_plugin::accesskit::ActionRequest,
    ) {
        let region = match region.region {
            EditorRegion::Window(_) => EditorRegion::Frame,
            region => region,
        };
        self.surface(region).node_action(request);
    }
}

#[cfg(test)]
mod tests;
