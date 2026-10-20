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

fn afresh(marks: &Marks, text: &Sequence<u8>) -> std::sync::Arc<Formats> {
    let mut fresh = Marks::default();
    for mark in marks.marks() {
        fresh.apply(text, mark.clone());
    }
    fresh.formats(text)
}

#[test]
fn rich_text_formats_kept_across_edits_match_formats_resolved_afresh() {
    let keys = [Name::new("bold"), Name::new("link"), Name::new("heading")];
    for seed in 0..40 {
        let mut random = Random(seed);
        let mut text = Sequence::from_items(b"one two\nthree four\nfive".to_vec());
        text.mirror();
        let mut marks = Marks::default();
        marks.formats(&text);
        for step in 0..120 {
            let len = text.len();
            let at = random.below(len + 1);
            let op = match random.below(4) {
                0 => text.insert(
                    1 + random.below(2) as u64,
                    at,
                    match random.below(3) {
                        0 => b"\n".to_vec(),
                        _ => b"xy".to_vec(),
                    },
                ),
                1 if len > 0 => text.delete(at.min(len - 1)..(at + 1 + random.below(4)).min(len)),
                _ => {
                    let bytes = text.items();
                    let end = (at + random.below(8)).min(len);
                    let key = &keys[random.below(keys.len())];
                    let (scope, expand) = match key.as_str() {
                        "heading" => (Scope::Block, Expand::After),
                        "link" => (Scope::Inline, Expand::Never),
                        _ => (Scope::Inline, Expand::After),
                    };
                    let value = (random.below(3) > 0).then(|| Value::Int(random.below(3) as i64));
                    if let Some(format) =
                        Marks::op(&text, &bytes, at..end, scope, expand, key, value)
                    {
                        marks.mint(&text, 1, format);
                    }
                    None
                }
            };
            if let Some(op) = op
                && let Some(splices) = text.apply(&op)
            {
                marks.splice(&text, &splices);
            }
            assert_eq!(
                marks.formats(&text),
                afresh(&marks, &text),
                "seed {seed}, step {step}: the kept formats drifted from a fresh resolve"
            );
        }
    }
}
