use beui::reactive::Get;
use beui::reactive::Set;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::marker::PhantomData;
use std::rc::Rc;

use beui_platform_plugin::{PluginView, Report};
use block_editor_plugin::{
    Artifact, ArtifactDescription, EditorHost, EditorRegion, Instance, Region,
};
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
    views: Rc<RefCell<Views<A>>>,
    keeps_accessibility: bool,
}

struct RegionApp<A: BeuiApp> {
    region: EditorRegion,
    host: EditorHost,
    views: Rc<RefCell<Views<A>>>,
    scale: Rc<Cell<BeuiScale>>,
    view: Option<PluginView>,
}

impl<A: BeuiApp> beui::App for RegionApp<A> {
    fn setup(&mut self, setup: &beui::Setup) {
        self.view = setup.get::<PluginView>().cloned();
    }

    fn update(&mut self, context: &beui::Context, rect: beui::Rect) {
        let Some(view) = &self.view else {
            return;
        };
        let Some(region) = view.region() else {
            return;
        };
        self.scale.set(BeuiScale {
            ratio: view.ratio(),
            pixels_per_point: context.pixels_per_point(),
        });
        let mut views = self.views.borrow_mut();
        let report = match self.region {
            EditorRegion::Frame if views.creating => Report {
                content: views.creation(context, rect),
                ..Report::default()
            },
            EditorRegion::Frame => {
                views.build_chrome();
                let picks = self.host.take_menu_picks();
                let (report, exit) = views.frame(context, rect, &region, &picks);
                if exit {
                    self.host.leave_frame();
                }
                report
            }
            EditorRegion::Preview => {
                views.preview(context, rect);
                Report::default()
            }
            EditorRegion::ArtifactSettings => {
                if let Some(mut draft) = self.host.settings_draft() {
                    views.artifact_settings(context, rect, &mut draft);
                    self.host.set_settings_draft(draft);
                }
                Report::default()
            }
        };
        view.report(report);
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
    ) -> (Report, bool) {
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
        let document = chrome.document();
        let report = Report {
            content: document.node_rect(chrome.content()),
            painted: vec![rect],
            floating: document.overlay_rects(),
            drawing: document.drawing_rects(),
            every_sample: document.wants_every_sample(),
            claims: document
                .press_claims()
                .into_iter()
                .map(|(modifiers, rect)| block_editor_plugin::Claim { modifiers, rect })
                .collect(),
        };
        (report, chrome.exit().take())
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
        {
            let mut views = instance.views.borrow_mut();
            match adopted {
                crate::headless::Adopted::Editor(editor, view) => {
                    views.editor = Some(editor);
                    views.view = view;
                }
                crate::headless::Adopted::Preview(editor) => views.preview = Some(editor),
                crate::headless::Adopted::Creation(creation) => views.creation = Some(creation),
                crate::headless::Adopted::Artifacts(artifacts) => {
                    views.artifacts = Some(artifacts);
                }
            }
        }
        instance
    }

    pub(crate) fn editor(&self) -> Option<Editor> {
        self.views.borrow().editor.as_deref().copied()
    }

    pub(crate) fn document(&self, region: EditorRegion) -> Option<Ref<'_, beui::Document>> {
        Ref::filter_map(self.views.borrow(), |views| {
            match (region, views.creating) {
                (EditorRegion::Frame, false) => views.chrome.as_ref().map(BeuiFrame::document),
                (EditorRegion::Frame, true) => views.dialog.as_ref(),
                (EditorRegion::Preview, _) => views.preview_document.as_ref(),
                (EditorRegion::ArtifactSettings, _) => views.settings.as_ref(),
            }
        })
        .ok()
    }

    pub(crate) fn document_mut(&self, region: EditorRegion) -> Option<RefMut<'_, beui::Document>> {
        RefMut::filter_map(self.views.borrow_mut(), |views| {
            match (region, views.creating) {
                (EditorRegion::Frame, false) => views.chrome.as_mut().map(BeuiFrame::document_mut),
                (EditorRegion::Frame, true) => views.dialog.as_mut(),
                (EditorRegion::Preview, _) => views.preview_document.as_mut(),
                (EditorRegion::ArtifactSettings, _) => views.settings.as_mut(),
            }
        })
        .ok()
    }

    fn new(host: EditorHost) -> Self {
        Self {
            host,
            scale: Rc::default(),
            views: Rc::new(RefCell::new(Views::empty())),
            keeps_accessibility: false,
        }
    }
}

impl<A: BeuiApp> Instance for BeuiInstance<A> {
    fn connect(&mut self, block_id: Uuid) {
        let mut views = self.views.borrow_mut();
        let fresh = |editor: &Option<OwnedEditor>| {
            editor
                .as_ref()
                .is_none_or(|editor| editor.block_id() != block_id)
        };
        let contents = [&views.editor, &views.preview]
            .into_iter()
            .flatten()
            .find(|editor| editor.block_id() == block_id)
            .map(OwnedEditor::contents)
            .unwrap_or_default();
        let scaled = |host: EditorHost| {
            OwnedEditor::scaled(host, block_id, Rc::clone(&self.scale), Rc::clone(&contents))
        };
        if fresh(&views.editor) {
            views.chrome = None;
            views.editor = Some(scaled(self.host.clone()));
            views.view = None;
        }
        if fresh(&views.preview) {
            views.preview_document = None;
            views.preview = Some(scaled(self.host.clone()));
        }
    }

    fn connect_creation(&mut self, template: String) {
        let mut views = self.views.borrow_mut();
        views.creating = true;
        let creation = views
            .creation
            .take()
            .unwrap_or_else(|| Creation::for_template(self.host.clone(), template));
        let built = creation.clone();
        views.dialog = Some(beui::reactive::build(move || {
            beui::reactive::view! {
                <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                    <A::Creation creation={built} />
                </beui::styled::Root>
            }
        }));
        views.creation = Some(creation);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let views = self.views.borrow();
        let creation = views
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        A::create_block(creation)
    }

    fn connect_artifact(&mut self, artifact: Artifact) {
        let mut views = self.views.borrow_mut();
        let artifacts = match views.artifacts.take() {
            Some(artifacts) if artifacts.block_id() == artifact.block_id => artifacts,
            _ => Artifacts::new(self.host.clone(), artifact),
        };
        A::connect_artifact(&artifacts);
        views.artifacts = Some(artifacts);
    }

    fn describe_artifact(&mut self, data: &[u8]) -> Result<ArtifactDescription, String> {
        A::describe_artifact(data)
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        if let Some(artifacts) = &self.views.borrow().artifacts {
            artifacts.regenerate(data);
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        self.views.borrow().artifacts.as_ref()?.poll()
    }

    fn intrinsic_size(&mut self) -> Option<beui::Vec2> {
        let views = self.views.borrow();
        views
            .editor
            .as_deref()
            .and_then(Editor::intrinsic_size)
            .or_else(|| views.preview.as_deref().and_then(Editor::intrinsic_size))
            .or_else(A::intrinsic_size)
    }

    fn resized(&mut self, size: beui::Vec2) {
        if let Some(editor) = &self.views.borrow().editor {
            editor.report_resize(size);
        }
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        A::aspect_ratio()
    }

    fn presence_visible(&mut self, visible: bool) {
        if let Some(editor) = &self.views.borrow().editor {
            editor.report_presence_visible(visible);
        }
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        let Some(editor) = self.editor() else {
            return false;
        };
        editor.replace_child(old, new)
    }

    fn open_region(&mut self, region: EditorRegion) {
        let options = beui::RunOptions {
            accessibility_tree: self.keeps_accessibility,
            ..beui::RunOptions::default()
        };
        let app = RegionApp::<A> {
            region,
            host: self.host.clone(),
            views: Rc::clone(&self.views),
            scale: Rc::clone(&self.scale),
            view: None,
        };
        let context = crate::fonts::context();
        if let Err(error) = beui_platform_plugin::launch(&self.host, region, options, context, app)
        {
            panic!("a beui region could not start: {error}");
        }
    }
}

#[cfg(test)]
mod tests;
