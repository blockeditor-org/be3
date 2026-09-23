use beui::{
    Color32, Context, Document, Event, Key, Modifiers, PointerButton, Pos2, Rect, TouchId,
    TouchPhase, Vec2,
};
use block_editor_plugin::beui_frame::BeuiFrame;
use block_editor_plugin::{
    Artifacts, BeuiApp, ChildPlacement, ChildStatus, Creation, Editor, EditorRegion, Occluder,
};
use std::marker::PhantomData;

use crate::snapshot;

mod capture;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);
const SETTLE_FRAMES: usize = 400;
const SETTLE_PAUSE: std::time::Duration = std::time::Duration::from_millis(2);
const MINIMUM_ZOOM: f32 = 1.0 / 64.0;
const MAXIMUM_ZOOM: f32 = 32.0;

pub struct BeuiTest<A: BeuiApp> {
    region: Region,
    context: Context,
    size: Vec2,
    pixels_per_point: f32,
    events: Vec<Event>,
    modifiers: Modifiers,
    draft: Vec<u8>,
    output: Option<beui::FrameOutput>,
    recording: Option<snapshot::Snapshot>,
    viewport: Option<Viewport>,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    app: PhantomData<A>,
}

enum Region {
    Frame(Editor, BeuiFrame),
    Preview(Editor, Document),
    Creation(Creation, Document),
    Settings(Artifacts, Document),
}

impl<A: BeuiApp> BeuiTest<A> {
    pub fn new(editor: Editor) -> Self {
        let frame = BeuiFrame::build({
            let editor = editor.clone();
            move || A::view(editor)
        });
        Self::for_region(Region::Frame(editor, frame))
    }

    pub fn with_view(editor: Editor, view: impl FnOnce() -> beui::NodeId) -> Self {
        let frame = BeuiFrame::build(view);
        Self::for_region(Region::Frame(editor, frame))
    }

    pub fn preview(editor: Editor) -> Self {
        let document = beui::reactive::build({
            let editor = editor.clone();
            move || A::preview_view(editor)
        });
        Self::for_region(Region::Preview(editor, document))
    }

    pub fn settings(artifacts: Artifacts, data: Vec<u8>) -> Self {
        let document = beui::reactive::build({
            let artifacts = artifacts.clone();
            move || A::artifact_settings_view(artifacts)
        });
        let mut editor = Self::for_region(Region::Settings(artifacts, document));
        editor.set_draft(data);
        editor.run();
        editor
    }

    pub fn draft(&self) -> &[u8] {
        &self.draft
    }

    pub fn set_draft(&mut self, data: Vec<u8>) {
        self.draft = data;
    }

    pub fn creation(creation: Creation) -> Self {
        let document = beui::reactive::build({
            let creation = creation.clone();
            move || A::creation_view(creation)
        });
        Self::for_region(Region::Creation(creation, document))
    }

    fn for_region(region: Region) -> Self {
        let context = Context::new();
        context.set_pixels_per_point(1.0);
        let mut editor = Self {
            region,
            context,
            size: SIZE,
            pixels_per_point: 1.0,
            events: Vec::new(),
            modifiers: Modifiers::NONE,
            draft: Vec::new(),
            output: None,
            recording: None,
            viewport: None,
            children: Vec::new(),
            occluders: Vec::new(),
            app: PhantomData,
        };
        editor.run();
        editor
    }

    pub fn in_viewport(mut self) -> Self {
        self.viewport = Some(Viewport::new());
        self.run();
        self
    }

    pub fn wants_another_frame(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(|output| output.changed || output.repaint_after < std::time::Duration::MAX)
    }

    pub fn document(&self) -> &Document {
        match &self.region {
            Region::Frame(_, frame) => frame.document(),
            Region::Preview(_, document)
            | Region::Creation(_, document)
            | Region::Settings(_, document) => document,
        }
    }

    pub fn children(&self) -> &[ChildPlacement] {
        &self.children
    }

    pub fn occluders(&self) -> &[Occluder] {
        &self.occluders
    }

    pub fn replace_child(&mut self, old: uuid::Uuid, new: uuid::Uuid) -> bool {
        let Region::Frame(editor, frame) = &mut self.region else {
            return false;
        };
        let editor = editor.clone();
        beui::reactive::with_reactive_scope(frame.document_mut(), move || {
            editor.replace_child(old, new)
        })
    }

    pub fn presence_visible(&mut self, visible: bool) {
        self.editor_handle()
            .expect("this region has no editor")
            .report_presence_visible(visible);
        self.run();
    }

    pub fn reveal_presence(&mut self, client_id: u64) {
        self.editor_handle()
            .expect("this region has no editor")
            .report_reveal(client_id);
        self.run();
    }

    pub fn resize(&mut self, size: Vec2) {
        self.editor_handle()
            .expect("this region has no editor")
            .report_resize(size);
        self.run();
    }

    fn editor_handle(&self) -> Option<&Editor> {
        match &self.region {
            Region::Frame(editor, _) | Region::Preview(editor, _) => Some(editor),
            Region::Creation(..) | Region::Settings(..) => None,
        }
    }

    pub fn report_children(&mut self, report: impl Fn(&ChildPlacement) -> ChildStatus) {
        let statuses: Vec<_> = self.children.iter().map(report).collect();
        self.editor_host().set_child_statuses(statuses);
    }

    pub fn available_children(&mut self) {
        let region = self.region();
        self.report_children(|placement| ChildStatus {
            instance: block_editor_plugin::EditorInstanceId(0),
            region,
            child: placement.child,
            available: true,
            intrinsic: None,
            aspect_ratio: None,
            hovered: false,
            active: false,
            interaction: block_editor_plugin::InteractionMode::Preview,
            capabilities: block_editor_plugin::EditorCapabilities::default(),
            resize: block_editor_plugin::ResizeMode::None,
            error: None,
        });
    }

    pub fn rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }

    pub fn run(&mut self) {
        let events = std::mem::take(&mut self.events);
        if events.is_empty() {
            return self.step(Vec::new());
        }
        for event in events {
            self.step(vec![event]);
        }
    }

    pub fn settle_until(&mut self, what: &str, ready: impl Fn(&Self) -> bool) {
        for _ in 0..SETTLE_FRAMES {
            self.run();
            if ready(self) {
                return;
            }
            std::thread::sleep(SETTLE_PAUSE);
        }
        panic!("the editor drew {SETTLE_FRAMES} frames and is still waiting for {what}");
    }

    pub fn step(&mut self, events: Vec<Event>) {
        let rect = self.rect();
        let context = self.context.clone();
        let placement = self.region();
        let host = self.editor_host();
        let intrinsic = self.intrinsic();
        if let Some(viewport) = &mut self.viewport {
            viewport.place(&host, rect, intrinsic);
        }
        host.begin_region(placement, beui::Vec2::ZERO);
        let region = &mut self.region;
        match region {
            Region::Frame(editor, frame) => {
                let editor = editor.clone();
                beui::reactive::with_reactive_scope(frame.document_mut(), move || {
                    editor.begin_frame();
                });
            }
            Region::Preview(editor, document) => {
                let editor = editor.clone();
                beui::reactive::with_reactive_scope(document, move || editor.begin_frame());
            }
            Region::Creation(creation, document) => {
                let creation = creation.clone();
                beui::reactive::with_reactive_scope(document, move || creation.begin_frame());
            }
            Region::Settings(artifacts, document) => {
                let artifacts = artifacts.clone();
                let draft = std::mem::take(&mut self.draft);
                beui::reactive::with_reactive_scope(document, move || {
                    artifacts.receive_settings(&draft);
                });
            }
        }
        let mut output = context.run(beui::RawInput { events }, |context| match region {
            Region::Frame(_, frame) => frame.document_mut().show(context, rect),
            Region::Preview(_, document)
            | Region::Creation(_, document)
            | Region::Settings(_, document) => document.show(context, rect),
        });
        match &self.region {
            Region::Frame(editor, frame) => editor.end_frame(frame.document()),
            Region::Preview(editor, document) => editor.end_frame(document),
            Region::Creation(..) => {}
            Region::Settings(artifacts, _) => {
                self.draft = artifacts
                    .take_settings_edit()
                    .unwrap_or_else(|| artifacts.settings().get_untracked());
            }
        }
        let (children, occluders) = self.editor_host().end_region(placement);
        let host = self.editor_host();
        host.grab_cursor(output.pointer_locked);
        if let Some(viewport) = &mut self.viewport {
            viewport.settle(&host, rect);
        }
        self.children = children;
        self.occluders = occluders;
        if let Some(delay) = host.take_frame_request() {
            output.repaint_after = output.repaint_after.min(delay);
        }
        self.output = Some(output);
    }

    pub fn pointer_locked(&self) -> bool {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .pointer_locked
    }

    pub fn pointer_motion(&mut self, delta: Vec2) {
        self.events.push(Event::PointerMotion(delta));
    }

    pub fn intrinsic_size(&self) -> Option<Vec2> {
        self.intrinsic()
    }

    fn intrinsic(&self) -> Option<Vec2> {
        match &self.region {
            Region::Frame(editor, _) | Region::Preview(editor, _) => editor.intrinsic_size(),
            Region::Creation(..) | Region::Settings(..) => None,
        }
    }

    fn region(&self) -> EditorRegion {
        match &self.region {
            Region::Preview(..) => EditorRegion::Preview,
            Region::Settings(..) => EditorRegion::ArtifactSettings,
            Region::Frame(..) | Region::Creation(..) => EditorRegion::Frame,
        }
    }

    fn editor_host(&self) -> block_editor_plugin::EditorHost {
        match &self.region {
            Region::Frame(editor, _) | Region::Preview(editor, _) => editor.host().clone(),
            Region::Creation(creation, _) => creation.host().clone(),
            Region::Settings(artifacts, _) => artifacts.host().clone(),
        }
    }

    pub fn hover_at(&mut self, pos: Pos2) {
        self.events.push(Event::PointerMoved(pos));
    }

    pub fn shown(&self, test_id: &str) -> bool {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .is_some()
    }

    pub fn label(&self, test_id: &str) -> String {
        let node = self
            .document()
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"));
        let mut collected = Vec::new();
        collect_text(self.document(), node, &mut collected);
        collected.join(" ")
    }

    pub fn rect_of(&self, test_id: &str) -> Rect {
        self.output
            .as_ref()
            .expect("the editor has not drawn a frame yet")
            .test_id_rect(test_id)
            .unwrap_or_else(|| panic!("no element with test id {test_id:?}"))
    }

    pub fn hover(&mut self, test_id: &str) {
        self.hover_at(self.rect_of(test_id).center());
    }

    pub fn click(&mut self, test_id: &str) {
        self.click_at(self.rect_of(test_id).center());
    }

    pub fn double_click(&mut self, test_id: &str) {
        self.double_click_at(self.rect_of(test_id).center());
    }

    pub fn double_click_at(&mut self, pos: Pos2) {
        self.click_at(pos);
        self.click_at(pos);
    }

    pub fn click_at(&mut self, pos: Pos2) {
        self.hover_at(pos);
        self.events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: self.modifiers,
        });
    }

    pub fn drag(&mut self, from: Pos2, to: Pos2) {
        self.hover_at(from);
        self.events.push(Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: self.modifiers,
        });
        self.events.push(Event::PointerMoved(to));
        self.events
            .push(Event::PointerMoved(to + Vec2::new(0.0, 0.5)));
        self.events.push(Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: self.modifiers,
        });
    }

    pub fn touch_start(&mut self, pos: Pos2) {
        self.touch(TouchPhase::Start, pos);
    }

    pub fn touch_move(&mut self, pos: Pos2) {
        self.touch(TouchPhase::Move, pos);
    }

    pub fn touch_end(&mut self, pos: Pos2) {
        self.touch(TouchPhase::End, pos);
    }

    pub fn touch_cancel(&mut self, pos: Pos2) {
        self.touch(TouchPhase::Cancel, pos);
    }

    fn touch(&mut self, phase: TouchPhase, pos: Pos2) {
        self.events.push(Event::Touch {
            id: TouchId {
                device: 1,
                finger: 1,
            },
            phase,
            pos,
            force: None,
        });
    }

    pub fn key_press(&mut self, key: Key) {
        self.key_press_modifiers(Modifiers::NONE, key);
    }

    pub fn key_press_modifiers(&mut self, modifiers: Modifiers, key: Key) {
        self.events.push(Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
        });
        self.events.push(Event::Key {
            key,
            pressed: false,
            repeat: false,
            modifiers,
        });
    }

    pub fn text(&mut self, text: impl Into<String>) {
        self.events.push(Event::Text(text.into()));
    }

    pub fn record(&mut self) {
        let frame = self.painted();
        match &mut self.recording {
            Some(recording) => recording.append(frame),
            None => self.recording = Some(frame),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        let painting = match self.recording.take() {
            Some(recording) => recording,
            None => self.painted(),
        };
        snapshot::assert_snapshot(name, &painting);
    }

    fn painted(&mut self) -> snapshot::Snapshot {
        let output = self
            .output
            .as_ref()
            .expect("the editor has not drawn a frame yet");
        capture::capture(output, self.size, self.pixels_per_point, Color32::BLACK)
            .expect("the painting could not be rendered")
    }
}

fn collect_text(document: &Document, node: beui::NodeId, collected: &mut Vec<String>) {
    if document.node_kind(node) == "text" {
        let detail = document.node_detail(node).unwrap_or_default();
        let text = detail.trim_matches('"');
        if !text.is_empty() {
            collected.push(text.to_owned());
        }
    }
    for child in document.children(node) {
        collect_text(document, child, collected);
    }
}

struct Viewport {
    zoom: f32,
    pan: Vec2,
    fitting: bool,
}

impl Viewport {
    fn new() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            fitting: true,
        }
    }

    fn place(
        &mut self,
        host: &block_editor_plugin::EditorHost,
        region: Rect,
        intrinsic: Option<Vec2>,
    ) {
        let content = intrinsic
            .unwrap_or(Vec2::ZERO)
            .max(region.size())
            .max(Vec2::new(1.0, 1.0));
        if self.fitting {
            self.zoom = (region.width() / content.x)
                .min(region.height() / content.y)
                .min(1.0)
                .clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
            self.pan = Vec2::ZERO;
        }
        let size = content * self.zoom;
        let center = region.center() + self.pan;
        let view = Rect::from_min_size(center - size * 0.5, size);
        host.set_beui_view(view, self.zoom);
    }

    fn settle(&mut self, host: &block_editor_plugin::EditorHost, region: Rect) {
        for change in host.take_view_changes() {
            if change != block_editor_plugin::ViewChange::ResumeAutoFit {
                self.fitting = false;
            }
            match change {
                block_editor_plugin::ViewChange::Pan { x, y } => self.pan += Vec2::new(x, y),
                block_editor_plugin::ViewChange::Zoom { factor, anchor } => {
                    let zoom = (self.zoom * factor).clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
                    let anchor =
                        anchor.map_or(region.center(), |(x, y)| Pos2::new(x, y)) - region.center();
                    self.pan = anchor - (anchor - self.pan) * (zoom / self.zoom);
                    self.zoom = zoom;
                }
                block_editor_plugin::ViewChange::Fit
                | block_editor_plugin::ViewChange::ResumeAutoFit => self.fitting = true,
            }
        }
    }
}
