use beui::{
    Color32, Context, Document, Event, Key, Modifiers, PointerButton, Pos2, Rect, TouchId,
    TouchPhase, Vec2,
};
use block_editor_plugin::beui_frame::BeuiFrame;
use block_editor_plugin::{BeuiApp, Creation, Editor};
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
    app: PhantomData<A>,
}

enum Region {
    Frame(Editor, BeuiFrame),
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
            app: PhantomData,
        };
        editor.run();
        editor
    }

    pub fn document(&self) -> &Document {
        match &self.region {
            Region::Frame(_, frame) => frame.document(),
            Region::Creation(_, document) => document,
        }
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
        let region = &mut self.region;
        match region {
            Region::Frame(editor, frame) => {
                let editor = editor.clone();
                beui::reactive::with_reactive_scope(frame.document_mut(), move || {
                    editor.begin_frame();
                });
            }
            Region::Creation(creation, document) => {
                let creation = creation.clone();
                beui::reactive::with_reactive_scope(document, move || creation.begin_frame());
            }
        }
        let output = context.run(beui::RawInput { events }, |context| match region {
            Region::Frame(_, frame) => frame.document_mut().show(context, rect),
            Region::Creation(_, document) => document.show(context, rect),
        });
        if let Region::Frame(editor, frame) = &self.region {
            editor.end_frame(frame.document());
        }
        self.output = Some(output);
    }

    pub fn hover_at(&mut self, pos: Pos2) {
        self.events.push(Event::PointerMoved(pos));
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
