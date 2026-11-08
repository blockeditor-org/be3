use super::*;
use crate::Stored;

/// Loading needs no session state to agree: every replica gives loaded content the same
/// positions (one reserved client at offsets `0..len`, list slots in stored order) and the same
/// ids, even for list items stored without one.
#[test]
fn replicas_loading_the_same_bytes_share_positions_and_exchange_edits() {
    let mut stored = note("hello").to_stored();
    let Stored::Map(entries) = &mut stored else {
        panic!("a document is a map");
    };
    let Some((_, Stored::Map(root))) = entries
        .iter_mut()
        .find(|(key, _)| key.as_text() == Some("root"))
    else {
        panic!("the root is a map");
    };
    let Some((_, Stored::Array(lines))) = root
        .iter_mut()
        .find(|(key, _)| key.as_text() == Some("lines"))
    else {
        panic!("lines are a list");
    };
    let Stored::Map(line) = &mut lines[0] else {
        panic!("a line is a map");
    };
    line.retain(|(key, _)| key.as_text() != Some("$id"));
    let bytes = crate::stored::encode(&stored);

    let mut first = Document::<Note>::from_bytes(&bytes).expect("the bytes decode");
    let mut second = Document::<Note>::from_bytes(&bytes).expect("the bytes decode");
    assert_eq!(first, second);
    let line = first.root().lines[0].id;
    assert_eq!(second.root().lines[0].id, line);

    let typed = typed_into(&mut first, 5, " world");
    let words = first.text(line, Line::WORDS).expect("the line has words");
    let line_typed: Edit = Line::WORDS
        .edit(
            line,
            words
                .insert(ALICE, 0, b"the ".to_vec())
                .expect("there is something to insert"),
        )
        .into();
    first.apply(&line_typed);

    crate::set_local_client(2);
    let prefixed = body_edit(&second, |body| body.insert(2, 0, b"> ".to_vec()));
    second.apply(&prefixed);
    let (_, added) = Note::LINES.insert(
        ObjectId::ROOT,
        Anchor::After(line),
        &Line {
            words: Text::from("second"),
        },
    );
    let added: Edit = added.into();
    second.apply(&added);

    for edit in [&prefixed, &added] {
        first.apply(edit);
    }
    for edit in [&typed, &line_typed] {
        second.apply(edit);
    }

    assert_eq!(body(&first), "> hello world");
    let lines: Vec<String> = first
        .root()
        .lines
        .iter()
        .map(|line| line.words.to_str_lossy().into_owned())
        .collect();
    assert_eq!(lines, ["the first", "second"]);
    assert_eq!(first, second);
    assert_eq!(first.session_state(), second.session_state());
}
