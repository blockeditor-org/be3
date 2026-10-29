use beui::Overflow;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use beui_host::native::Document;
use beui_host::native::build;
use beui_host::native::enter;
use beui_host::native::testing::{Screen, Showing};
use beui::primitives::{Span, Text};
use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::TextAreaState;
use beui::{
    Input, PointerButton, Pos2, Vec2, native::Incoming, native::Launch, native::PlatformServices,
    native::RunOptions, native::Setup, native::Waker, pos2,
};
use beui_core::app::SafeArea;
use beui_core::renderer::Loaded;
use beui_core::runner::Runner;
use text_editor_core::TextBuffer;

const VIEWPORT: Vec2 = Vec2::new(1000.0, 800.0);
const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
const WARMUP: usize = 5;
const PARAGRAPH: &str = "A text node lays out on one line unless it wraps, and a line that does \
    not fit can end in an ellipsis instead of spilling out of its frame. Spans give colour, size \
    and font to the parts of one text, and a change to one of them shapes only that one again. ";

struct Bare;

impl PlatformServices for Bare {}

struct Bench {
    runner: Runner,
}

impl Bench {
    fn new(document: Document) -> Self {
        let mut runner = Runner::new(Launch {
            options: RunOptions {
                size: VIEWPORT,
                ..RunOptions::default()
            },
            context: beui_host::native::context(),
            app: Box::new(Showing(document)),
            layers: Vec::new(),
        });
        runner
            .start(
                Loaded {
                    renderer: Box::new(Screen(VIEWPORT)),
                    fonts: None,
                },
                Setup::new(Waker::new(|| {})),
            )
            .expect("a screen without a gpu always starts");
        runner.context().stop_clock();
        Self { runner }
    }

    fn frame(&mut self, inputs: Vec<Input>) -> Duration {
        self.runner.context().advance_clock(FRAME_INTERVAL);
        for input in inputs {
            self.runner.push(Incoming::points(input));
        }
        let started = Instant::now();
        self.runner.frame(&mut Bare, SafeArea::default());
        let took = started.elapsed();
        if let Some(failure) = self.runner.failure() {
            panic!("the bench panicked: {failure}");
        }
        took
    }

    fn update(&mut self, change: impl FnOnce()) {
        let app: &mut dyn Any = self.runner.app_mut();
        let Showing(document) = app.downcast_mut().expect("the bench shows a document");
        enter(document, change);
    }

    fn click(&mut self, pos: Pos2) {
        self.frame(vec![Input::PointerMoved(pos)]);
        for pressed in [true, false] {
            self.frame(vec![Input::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
            }]);
        }
    }
}

fn writer<T: 'static>() -> Rc<RefCell<Option<Signal<T>>>> {
    Rc::new(RefCell::new(None))
}

fn taken<T: Clone>(slot: &Rc<RefCell<Option<T>>>) -> T {
    slot.borrow().clone().expect("the document was built")
}

struct Report {
    name: &'static str,
    first: Duration,
    frames: Vec<Duration>,
}

impl Report {
    fn print(mut self) {
        self.frames.sort();
        let count = self.frames.len();
        let total: Duration = self.frames.iter().sum();
        let at = |share: f64| self.frames[((count - 1) as f64 * share) as usize];
        bench::time(&format!("{}.first", self.name), self.first);
        bench::time(&format!("{}.median", self.name), at(0.5));
        bench::time(&format!("{}.p90", self.name), at(0.9));
        println!(
            "{:<24} first {:>9.1?}  mean {:>9.1?}  median {:>9.1?}  p90 {:>9.1?}  ({count} frames)",
            self.name,
            self.first,
            total / count as u32,
            at(0.5),
            at(0.9),
        );
    }
}

fn measure(
    name: &'static str,
    frames: usize,
    document: Document,
    prepare: impl FnOnce(&mut Bench),
    mut step: impl FnMut(&mut Bench, usize) -> Vec<Input>,
    check: impl FnOnce(),
) -> Report {
    let mut bench = Bench::new(document);
    let first = bench.frame(Vec::new());
    prepare(&mut bench);
    for index in 0..WARMUP {
        let events = step(&mut bench, index);
        bench.frame(events);
    }
    let timed = (WARMUP..WARMUP + frames)
        .map(|index| {
            let events = step(&mut bench, index);
            bench.frame(events)
        })
        .collect();
    check();
    Report {
        name,
        first,
        frames: timed,
    }
}

fn resize_paragraphs(frames: usize) -> Report {
    let width = writer::<f32>();
    let document = build({
        let width = width.clone();
        move || {
            let read = create_signal(VIEWPORT.x);
            *width.borrow_mut() = Some(read);
            let text = PARAGRAPH.repeat(4);
            view! {
                <Root>
                    <Frame width={create_memo(move || Length::Size(Size::Custom(read.get())))}>
                        <List spacing=Space::Md>
                            <ForEach keys={(0..20).collect::<Vec<usize>>()}>|_: usize| {
                                let text = text.clone(); view! {
                                    <Text>{text}</Text>
                                }
                            }</ForEach>
                        </List>
                    </Frame>
                </Root>
            }
        }
    });
    let width = taken(&width);
    measure(
        "resize-paragraphs",
        frames,
        document,
        |_| {},
        move |bench, index| {
            let next = 300.0 + (index * 37 % 700) as f32;
            bench.update(|| width.set(next));
            Vec::new()
        },
        || {},
    )
}

fn resize_labels(frames: usize) -> Report {
    let width = writer::<f32>();
    let document = build({
        let width = width.clone();
        move || {
            let read = create_signal(VIEWPORT.x);
            *width.borrow_mut() = Some(read);
            view! {
                <Root>
                    <Frame width={create_memo(move || Length::Size(Size::Custom(read.get())))}>
                        <List spacing=Space::None>
                            <ForEach keys={(0..2000).collect::<Vec<usize>>()}>|index: usize|
                                <Text overflow=Overflow::Ellipsis>
                                    {format!("label number {index}")}
                                </Text>
                            </ForEach>
                        </List>
                    </Frame>
                </Root>
            }
        }
    });
    let width = taken(&width);
    measure(
        "resize-labels",
        frames,
        document,
        |_| {},
        move |bench, index| {
            let next = 100.0 + (index * 37 % 900) as f32;
            bench.update(|| width.set(next));
            Vec::new()
        },
        || {},
    )
}

fn edit_plain_paragraph(frames: usize) -> Report {
    let content = writer::<String>();
    let document = build({
        let content = content.clone();
        move || {
            let read = create_signal(PARAGRAPH.repeat(40));
            *content.borrow_mut() = Some(read);
            view! {
                <Root>
                    <Text>{read}</Text>
                </Root>
            }
        }
    });
    let content = taken(&content);
    measure(
        "edit-plain-paragraph",
        frames,
        document,
        |_| {},
        move |bench, index| {
            let mut text = PARAGRAPH.repeat(40);
            let at = (index * 997) % (text.len() / 2) * 2;
            let at = (at..text.len())
                .find(|at| text.is_char_boundary(*at))
                .unwrap_or(0);
            text.insert_str(at, "edit ");
            bench.update(|| content.set(text));
            Vec::new()
        },
        || {},
    )
}

fn edit_one_span(frames: usize) -> Report {
    let words = writer::<Vec<String>>();
    let count = PARAGRAPH.split_inclusive(' ').count() * 40;
    let initial: Vec<String> = PARAGRAPH
        .split_inclusive(' ')
        .cycle()
        .take(count)
        .map(str::to_owned)
        .collect();
    let document = build({
        let words = words.clone();
        let initial = initial.clone();
        move || {
            let read = create_signal(initial.clone());
            *words.borrow_mut() = Some(read);
            let keys = (0..initial.len()).collect::<Vec<usize>>();
            view! {
                <Root>
                    <Text>
                        <ForEach keys={keys}>|index: usize| {
                            let word = create_memo(move || {
                                read.get().get(index).cloned().unwrap_or_default()
                            });
                            view! {
                                <Span>{word}</Span>
                            }
                        }</ForEach>
                    </Text>
                </Root>
            }
        }
    });
    let words = taken(&words);
    measure(
        "edit-one-span",
        frames,
        document,
        |_| {},
        move |bench, index| {
            let mut next = initial.clone();
            next[(index * 997) % count] = format!("edit{index} ");
            bench.update(|| words.set(next));
            Vec::new()
        },
        || {},
    )
}

fn markdown(lines: usize) -> String {
    let mut text = String::new();
    for line in 0..lines {
        let row = match line % 8 {
            0 => format!("# Heading {line}"),
            1 => format!("Some **bold** and *italic* words on line {line}, with `code` inside."),
            2 => format!("- [ ] a task on line {line}"),
            3 => format!("- a point that runs on for a while so that it wraps on line {line}"),
            4 => String::new(),
            5 => format!("A plain sentence on line {line} that says nothing much at all."),
            6 => format!("> a quote on line {line}"),
            _ => format!("    indented code {line}"),
        };
        text.push_str(&row);
        text.push('\n');
    }
    text
}

fn type_in_text_area(frames: usize) -> Report {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::default();
    let document = build({
        let held = held.clone();
        move || {
            let buffer = Arc::new(TextBuffer::new(markdown(3000).as_bytes()))
                as Arc<dyn text_editor_core::Document>;
            let state = TextAreaState::new(buffer);
            *held.borrow_mut() = Some(state);
            view! {
                <Root>
                    <TextArea state={state} />
                </Root>
            }
        }
    });
    let state = taken(&held);
    let before = state.bytes().len();
    measure(
        "type-in-text-area",
        frames,
        document,
        |bench| bench.click(pos2(300.0, 60.0)),
        |_, index| {
            let letter = match index % 6 {
                5 => ' ',
                step => char::from(b'a' + step as u8),
            };
            vec![Input::Text(letter.to_string())]
        },
        move || {
            assert_eq!(
                state.bytes().len(),
                before + WARMUP + frames,
                "every frame typed one letter into the text area"
            );
        },
    )
}

type Scenario = (&'static str, fn(usize) -> Report);

const SCENARIOS: [Scenario; 5] = [
    ("resize-paragraphs", resize_paragraphs),
    ("resize-labels", resize_labels),
    ("edit-plain-paragraph", edit_plain_paragraph),
    ("edit-one-span", edit_one_span),
    ("type-in-text-area", type_in_text_area),
];

fn main() {
    let mut frames = 200;
    let mut only = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--frames" => {
                frames = arguments
                    .next()
                    .and_then(|count| count.parse().ok())
                    .expect("--frames takes a count");
            }
            name => only.push(name.to_owned()),
        }
    }
    for (name, run) in SCENARIOS {
        if only.is_empty() || only.iter().any(|wanted| name.contains(wanted.as_str())) {
            run(frames).print();
        }
    }
}
