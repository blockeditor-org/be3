use beui::{
    Color32, Context, Document, Event, Key, Modifiers, PointerButton, Pos2, Rect, TouchId,
    TouchPhase, Vec2,
};
use block_editor_plugin::beui_frame::BeuiFrame;
use block_editor_plugin::{BeuiApp, ChildPlacement, ChildStatus, Creation, Editor, EditorRegion};
use std::marker::PhantomData;

use crate::snapshot;

mod capture;

const SIZE: Vec2 = Vec2::new(800.0, 600.0);

pub struct BeuiTest<A: BeuiApp> {
    region: Region,
    context: Context,
    size: Vec2,
    pixels_per_point: f32,
    events: Vec<Event>,
    modifiers: Modifiers,
    output: Option<beui::FrameOutput>,
    children: Vec<ChildPlacement>,
    app: PhantomData<A>,
}

enum Region {
    Frame(Editor, BeuiFrame),
    Preview(Editor, Document),
    Creation(Creation, Document),
}

impl<A: BeuiApp> BeuiTest<A> {
    pub fn new(editor: Editor) -> Self {
        let frame = BeuiFrame::build({
            let editor = editor.clone();
            move || A::view(editor)
        });
        Self::for_region(Region::Frame(editor, frame))
    }

    pub fn preview(editor: Editor) -> Self {
        let document = beui::reactive::build({
            let editor = editor.clone();
            move || A::preview_view(editor)
        });
        Self::for_region(Region::Preview(editor, document))
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
            output: None,
            children: Vec::new(),
            app: PhantomData,
        };
        editor.run();
        editor
    }

    pub fn document(&self) -> &Document {
        match &self.region {
            Region::Frame(_, frame) => frame.document(),
            Region::Preview(_, document) | Region::Creation(_, document) => document,
        }
    }

    pub fn children(&self) -> &[ChildPlacement] {
        &self.children
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
            intrinsic_width: 0.0,
            intrinsic_height: 0.0,
            aspect_ratio: 0.0,
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

    pub fn step(&mut self, events: Vec<Event>) {
        let rect = self.rect();
        let context = self.context.clone();
        let placement = self.region();
        self.editor_host()
            .begin_region(placement, block_editor_plugin::egui::Vec2::ZERO);
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
        }
        let output = context.run(beui::RawInput { events }, |context| match region {
            Region::Frame(_, frame) => frame.document_mut().show(context, rect),
            Region::Preview(_, document) | Region::Creation(_, document) => {
                document.show(context, rect)
            }
        });
        match &self.region {
            Region::Frame(editor, frame) => editor.end_frame(frame.document()),
            Region::Preview(editor, document) => editor.end_frame(document),
            Region::Creation(..) => {}
        }
        let (children, _) = self.editor_host().end_region(placement);
        self.children = children;
        self.output = Some(output);
    }

    fn region(&self) -> EditorRegion {
        match &self.region {
            Region::Preview(..) => EditorRegion::Preview,
            Region::Frame(..) | Region::Creation(..) => EditorRegion::Frame,
        }
    }

    fn editor_host(&self) -> block_editor_plugin::EditorHost {
        match &self.region {
            Region::Frame(editor, _) | Region::Preview(editor, _) => editor.host().clone(),
            Region::Creation(creation, _) => creation.host().clone(),
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

    pub fn snapshot(&mut self, name: &str) {
        let output = self
            .output
            .as_ref()
            .expect("the editor has not drawn a frame yet");
        let painting = capture::capture(output, self.size, self.pixels_per_point, Color32::BLACK)
            .expect("the painting could not be rendered");
        snapshot::assert_snapshot(name, &painting);
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
