use beui::reactive::Get;
use beui::reactive::Set;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use beui::reactive::RenderFn;
use beui::styled::WindowHandle;
use beui::{Document, WindowId};
use beui_platform_plugin::{Report, View};
use block_editor_plugin::{
    Artifact, ArtifactDescription, EditorHost, EditorInstanceId, EditorRegion, Instance,
};
use uuid::Uuid;

use crate::beui_frame::{self, BeuiFrame, FrameBar, FrameChrome};
use crate::editor::BeuiScale;
use crate::module::{Module, Shown};
use crate::{Artifacts, BeuiApp, Creation, Editor, OwnedEditor};

pub struct BeuiPlugin<A: BeuiApp>(PhantomData<A>);

impl<A: BeuiApp> block_editor_plugin::Plugin for BeuiPlugin<A> {
    fn open(host: EditorHost) -> Box<dyn Instance> {
        Box::new(BeuiInstance::<A>::new(host))
    }
}

pub(crate) struct BeuiInstance<A: BeuiApp> {
    host: EditorHost,
    parts: Rc<Parts<A>>,
    keeps_accessibility: bool,
    shown: Option<(Rc<Module>, EditorInstanceId)>,
}

struct Parts<A: BeuiApp> {
    host: EditorHost,
    dialog: Rc<Cell<Option<beui::NodeId>>>,
    scale: Rc<Cell<BeuiScale>>,
    views: RefCell<Views<A>>,
    generation: beui::reactive::OwnedSignal<u64>,
}

struct Views<A: BeuiApp> {
    creating: bool,
    chrome: Rc<BeuiFrame>,
    view: Option<crate::headless::View>,
    editor: Option<OwnedEditor>,
    preview: Option<OwnedEditor>,
    creation: Option<Creation>,
    artifacts: Option<Artifacts>,
    app: PhantomData<A>,
}

impl<A: BeuiApp> Views<A> {
    fn empty() -> Self {
        Self {
            creating: false,
            chrome: Rc::default(),
            editor: None,
            preview: None,
            creation: None,
            artifacts: None,
            view: None,
            app: PhantomData,
        }
    }
}

impl<A: BeuiApp> Parts<A> {
    fn rebuild(&self) {
        self.generation
            .handle()
            .update(|generation| *generation += 1);
    }

    fn frame_content(&self) -> RenderFn<WindowHandle> {
        let mut views = self.views.borrow_mut();
        if views.creating {
            let Some(creation) = views.creation.clone() else {
                return empty();
            };
            let dialog = Rc::clone(&self.dialog);
            return RenderFn::new(move |_: WindowHandle| {
                let creation = creation.clone();
                let node = beui::reactive::view! {
                    <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                        <A::Creation creation={creation} />
                    </beui::styled::Root>
                }
                .child();
                dialog.set(Some(node));
                node
            });
        }
        let Some(editor) = views.editor.as_deref().copied() else {
            return empty();
        };
        let custom = Rc::new(RefCell::new(views.view.take()));
        let chrome = Rc::clone(&views.chrome);
        RenderFn::new(move |_: WindowHandle| {
            let custom = Rc::clone(&custom);
            let view: Rc<dyn Fn() -> beui::styled::Root> =
                Rc::new(move || match custom.borrow_mut().take() {
                    Some(view) => view(),
                    None => beui::reactive::view! {
                        <A::View editor />
                    },
                });
            beui::reactive::view! {
                <FrameChrome chrome={Rc::clone(&chrome)} editor view />
            }
            .child()
        })
    }

    fn preview_content(&self) -> RenderFn<WindowHandle> {
        let Some(editor) = self.views.borrow().preview.as_deref().copied() else {
            return empty();
        };
        RenderFn::new(move |_: WindowHandle| {
            beui::reactive::view! {
                <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                    <A::Preview editor />
                </beui::styled::Root>
            }
            .child()
        })
    }

    fn settings_content(&self) -> RenderFn<WindowHandle> {
        let Some(artifacts) = self.views.borrow().artifacts.clone() else {
            return empty();
        };
        RenderFn::new(move |_: WindowHandle| {
            let artifacts = artifacts.clone();
            beui::reactive::view! {
                <beui::styled::Root clear_color=beui::styled::ColorRole::None>
                    <A::ArtifactSettings artifacts={artifacts} />
                </beui::styled::Root>
            }
            .child()
        })
    }

    fn frame_report(
        &self,
        view: &View,
        document: &mut Document,
        window: WindowId,
    ) -> (Report, bool) {
        let report = Report {
            content: self
                .views
                .borrow()
                .chrome
                .content()
                .and_then(|content| document.node_rect(content)),
            ..self.window_report(view, document, window)
        };
        (report, self.views.borrow().chrome.exit().take())
    }

    fn window_report(&self, view: &View, document: &mut Document, window: WindowId) -> Report {
        if let Some(editor) = self.views.borrow().editor.as_deref() {
            editor.end_frame(document, window);
        }
        Report {
            content: None,
            painted: vec![beui::Rect::from_min_size(beui::Pos2::ZERO, view.size)],
            floating: document.overlay_rects(window),
            drawing: document.drawing_rects(window),
            every_sample: document.wants_every_sample(window),
            claims: document
                .press_claims(window)
                .into_iter()
                .map(|(modifiers, rect)| block_editor_plugin::Claim { modifiers, rect })
                .collect(),
        }
    }
}

fn empty() -> RenderFn<WindowHandle> {
    RenderFn::new(|_: WindowHandle| {
        beui::reactive::view! {
            <beui::styled::Frame />
        }
    })
}

impl<A: BeuiApp> Shown for Parts<A> {
    fn host(&self) -> EditorHost {
        self.host.clone()
    }

    fn generation(&self) -> u64 {
        self.generation.get()
    }

    fn content(&self, region: EditorRegion) -> RenderFn<WindowHandle> {
        match region {
            EditorRegion::Frame => self.frame_content(),
            EditorRegion::Preview => self.preview_content(),
            EditorRegion::ArtifactSettings => self.settings_content(),
            EditorRegion::Window(_) => empty(),
        }
    }

    fn before(&self, view: &View, context: &beui::Context, document: &mut Document) {
        self.scale.set(BeuiScale {
            ratio: view.ratio,
            pixels_per_point: context.pixels_per_point(),
        });
        let views = self.views.borrow();
        match view.id.region {
            EditorRegion::Frame if views.creating => {
                if let Some(creation) = &views.creation {
                    creation.begin_frame();
                }
            }
            EditorRegion::Frame => {
                let spec = &view.spec;
                views.chrome.bar().set(FrameBar {
                    shown: view.chrome_drawn() && (spec.top_bar.shown() || spec.content.is_some()),
                    framed: view.chrome_drawn(),
                    closable: spec.content.is_some(),
                    on_phone: spec.top_bar.phone(),
                });
                let picks = self.host.take_menu_picks();
                if !picks.is_empty() {
                    let menu = views.chrome.menu();
                    beui::reactive::with_reactive_scope(document, || {
                        for id in &picks {
                            beui_frame::run_menu_pick(&menu, id);
                        }
                    });
                }
                if let Some(editor) = views.editor.as_deref() {
                    editor.begin_frame();
                }
            }
            EditorRegion::Preview => {
                if let Some(editor) = views.preview.as_deref() {
                    editor.begin_frame();
                }
            }
            EditorRegion::ArtifactSettings => {
                if let (Some(artifacts), Some(draft)) =
                    (views.artifacts.as_ref(), self.host.settings_draft())
                {
                    artifacts.receive_settings(&draft);
                }
            }
            EditorRegion::Window(_) => {}
        }
    }

    fn after(
        &self,
        view: &View,
        context: &beui::Context,
        document: &mut Document,
        window: WindowId,
    ) -> Report {
        let creating = self.views.borrow().creating;
        match view.id.region {
            EditorRegion::Frame if creating => {
                let Some(dialog) = self.dialog.get() else {
                    return Report::default();
                };
                let width = view.size.x;
                let measured =
                    document.measure_node(context, dialog, beui::vec2(width, f32::INFINITY));
                Report {
                    content: measured
                        .filter(|measured| measured.y.is_finite())
                        .map(|measured| {
                            beui::Rect::from_min_size(
                                beui::Pos2::ZERO,
                                beui::vec2(width, measured.y),
                            )
                        }),
                    ..Report::default()
                }
            }
            EditorRegion::Frame => {
                let (report, exit) = self.frame_report(view, document, window);
                if exit {
                    self.host.leave_frame();
                }
                report
            }
            EditorRegion::Preview => {
                if let Some(editor) = self.views.borrow().preview.as_deref() {
                    editor.end_frame(document, window);
                }
                Report::default()
            }
            EditorRegion::ArtifactSettings => {
                if let Some(artifacts) = self.views.borrow().artifacts.as_ref() {
                    let draft = artifacts
                        .take_settings_edit()
                        .unwrap_or_else(|| artifacts.settings().get_untracked());
                    self.host.set_settings_draft(draft);
                }
                Report::default()
            }
            EditorRegion::Window(window) => self.window_report(view, document, window),
        }
    }
}

impl<A: BeuiApp> BeuiInstance<A> {
    pub(crate) fn adopting(adopted: crate::headless::Adopted) -> Self {
        let mut instance = Self::new(adopted.host());
        instance.keeps_accessibility = true;
        {
            let mut views = instance.parts.views.borrow_mut();
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
        self.parts.views.borrow().editor.as_deref().copied()
    }

    fn new(host: EditorHost) -> Self {
        Self {
            parts: Rc::new(Parts {
                host: host.clone(),
                dialog: Rc::default(),
                scale: Rc::default(),
                views: RefCell::new(Views::empty()),
                generation: beui::reactive::OwnedSignal::new(0),
            }),
            host,
            keeps_accessibility: false,
            shown: None,
        }
    }
}

impl<A: BeuiApp> Drop for BeuiInstance<A> {
    fn drop(&mut self) {
        if let Some((module, instance)) = self.shown.take() {
            module.hide(instance);
        }
    }
}

impl<A: BeuiApp> Instance for BeuiInstance<A> {
    fn connect(&mut self, block_id: Uuid) {
        let mut views = self.parts.views.borrow_mut();
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
            OwnedEditor::scaled(
                host,
                block_id,
                Rc::clone(&self.parts.scale),
                Rc::clone(&contents),
            )
        };
        let mut rebuilt = false;
        if fresh(&views.editor) {
            views.editor = Some(scaled(self.host.clone()));
            views.view = None;
            views.chrome = Rc::default();
            rebuilt = true;
        }
        if fresh(&views.preview) {
            views.preview = Some(scaled(self.host.clone()));
            rebuilt = true;
        }
        drop(views);
        if rebuilt {
            self.parts.rebuild();
        }
    }

    fn connect_creation(&mut self, template: String) {
        let mut views = self.parts.views.borrow_mut();
        views.creating = true;
        let creation = views
            .creation
            .take()
            .unwrap_or_else(|| Creation::for_template(self.host.clone(), template));
        views.creation = Some(creation);
        drop(views);
        self.parts.rebuild();
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let views = self.parts.views.borrow();
        let creation = views
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        A::create_block(creation)
    }

    fn connect_artifact(&mut self, artifact: Artifact) {
        let mut views = self.parts.views.borrow_mut();
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
        if let Some(artifacts) = &self.parts.views.borrow().artifacts {
            artifacts.regenerate(data);
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        self.parts.views.borrow().artifacts.as_ref()?.poll()
    }

    fn intrinsic_size(&mut self) -> Option<beui::Vec2> {
        let views = self.parts.views.borrow();
        views
            .editor
            .as_deref()
            .and_then(Editor::intrinsic_size)
            .or_else(|| views.preview.as_deref().and_then(Editor::intrinsic_size))
            .or_else(A::intrinsic_size)
    }

    fn resized(&mut self, size: beui::Vec2) {
        if let Some(editor) = &self.parts.views.borrow().editor {
            editor.report_resize(size);
        }
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        A::aspect_ratio()
    }

    fn presence_visible(&mut self, visible: bool) {
        if let Some(editor) = &self.parts.views.borrow().editor {
            editor.report_presence_visible(visible);
        }
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        let Some(editor) = self.editor() else {
            return false;
        };
        editor.replace_child(old, new)
    }

    fn open_region(&mut self, _region: EditorRegion) {
        if self.shown.is_some() {
            return;
        }
        let (Some(module), Some(instance)) = (Module::of(&self.host), self.host.instance()) else {
            return;
        };
        let parts: Rc<dyn Shown> = self.parts.clone();
        module.show(instance, parts, &self.host, self.keeps_accessibility);
        self.shown = Some((module, instance));
    }
}

#[cfg(test)]
mod tests;
