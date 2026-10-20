use super::*;

struct Random(u64);

impl Random {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

const TEXT: &str = "Title\nSome bold and plain text.\nfirst item\nsecond item\nnested\nA quote.\nfn main() {\n    body();\n}\nAsk \u{fffc} here.\n";

fn seeded() -> EditorTester {
    let line = |words: &str| {
        let start = TEXT.find(words).unwrap();
        start..start + words.len()
    };
    let void = TEXT.find(VOID).unwrap();
    let seeds = [
        schema::basic::block(line("Title"), "heading", Value::Int(1)),
        schema::basic::mark(line("bold"), "bold"),
        schema::basic::block(
            line("first item").start..line("nested").end,
            "list",
            Value::text("number"),
        ),
        schema::basic::block(line("nested"), "indent", Value::Int(1)),
        schema::basic::block(line("A quote."), "quote", Value::Bool(true)),
        schema::basic::block(
            line("fn main").start..line("\n}\n").start + 2,
            "code_block",
            Value::text("rust"),
        ),
        schema::basic::mark_value(void..void + VOID.len(), "mention", Value::text("ada")),
    ];
    let mut editor = Core::new(Arc::new(TextBuffer::rich(TEXT, &seeds)));
    editor.set_schema(Arc::new(schema::basic::Basic));
    let start = editor.position(0);
    editor.select(Selection::at(start));
    EditorTester { editor }
}

const TOOLS: [&str; 12] = [
    "bold",
    "italic",
    "underline",
    "strikethrough",
    "code",
    "heading1",
    "heading2",
    "bullet",
    "number",
    "check",
    "quote",
    "code_block",
];

fn step(tester: &mut EditorTester, random: &mut Random) -> String {
    let len = tester.bytes().len();
    let mut at = || random.below(len + 1);
    let (anchor, focus) = (at(), at());
    let collapsed = random.below(2) == 0;
    let focus = if collapsed { anchor } else { focus };
    let place = |tester: &mut EditorTester| {
        tester.execute(EditorCommand::SetSelection {
            anchor: tester.pos(anchor),
            focus: tester.pos(focus),
        });
    };
    let action = random.below(11);
    let left = |direction| EditorCommand::Delete {
        direction,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    };
    let at_line_start = random.below(2) == 0;
    let snap_to_line = |tester: &mut EditorTester| {
        let bytes = tester.bytes();
        let start = bytes[..anchor.min(bytes.len())]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |newline| newline + 1);
        let end = bytes[anchor.min(bytes.len())..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |newline| anchor + newline);
        let at = if anchor % 2 == 0 { end } else { start };
        tester.set_cursor(tester.pos(at));
        at
    };
    match action {
        0 => {
            place(tester);
            tester.execute(EditorCommand::InsertText(b"x"));
            format!("type x at {anchor}..{focus}")
        }
        1 => {
            place(tester);
            tester.execute(EditorCommand::Newline);
            format!("enter at {anchor}..{focus}")
        }
        2 | 3 if at_line_start => {
            let at = snap_to_line(tester);
            tester.execute(left(if action == 2 {
                LRDirection::Left
            } else {
                LRDirection::Right
            }));
            format!(
                "{} at line edge {at}",
                if action == 2 { "backspace" } else { "delete" }
            )
        }
        2 => {
            place(tester);
            tester.execute(left(LRDirection::Left));
            format!("backspace at {anchor}..{focus}")
        }
        3 => {
            place(tester);
            tester.execute(left(LRDirection::Right));
            format!("delete at {anchor}..{focus}")
        }
        4 | 5 => {
            place(tester);
            let name = TOOLS[random.below(TOOLS.len())];
            tool(tester, name);
            format!("{name} at {anchor}..{focus}")
        }
        6 => {
            place(tester);
            tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
            format!("tab at {anchor}..{focus}")
        }
        7 => {
            place(tester);
            tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
            format!("shift tab at {anchor}..{focus}")
        }
        8 => {
            place(tester);
            tester.execute(EditorCommand::Paste(b"pasted\ntwo"));
            format!("paste two lines at {anchor}..{focus}")
        }
        9 => {
            place(tester);
            tester.execute(EditorCommand::InsertVoid(MarkSet::new([(
                Name::new("mention"),
                Value::text("grace"),
            )])));
            format!("mention at {anchor}..{focus}")
        }
        _ => {
            place(tester);
            tester.execute(EditorCommand::InsertText(b" "));
            format!("type space at {anchor}..{focus}")
        }
    }
}

#[test]
fn rich_text_undo_and_redo_restore_every_random_edit() {
    for seed in 0..60 {
        let mut random = Random(seed);
        let mut tester = seeded();
        let start = formatted(&tester);
        let mut log: Vec<String> = Vec::new();
        for _ in 0..40 {
            let before = formatted(&tester);
            let described = step(&mut tester, &mut random);
            log.push(described);
            let after = formatted(&tester);
            if after == before {
                continue;
            }
            tester.execute(EditorCommand::Undo);
            assert_eq!(
                formatted(&tester),
                before,
                "seed {seed}: undo did not restore the document after {log:#?}"
            );
            tester.execute(EditorCommand::Redo);
            assert_eq!(
                formatted(&tester),
                after,
                "seed {seed}: redo did not restore the edit after {log:#?}"
            );
        }
        for _ in 0..log.len() {
            tester.execute(EditorCommand::Undo);
        }
        assert_eq!(
            formatted(&tester),
            start,
            "seed {seed}: undoing everything did not restore the start after {log:#?}"
        );
    }
}
