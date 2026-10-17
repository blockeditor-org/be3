use super::*;

#[test]
fn ime_composition_excludes_text_inserted_after_its_last_byte() {
    for suffix in ["", " there"] {
        let buffer = Arc::new(TextBuffer::new(format!("say {suffix}")));
        let mut core = Core::new(buffer.clone());
        core.select(Selection::at(core.position(4)));
        core.execute_command(EditorCommand::Ime(ImeCommand::SetComposingText("helo")));
        assert_eq!(core.composition(), Some(4..8));

        buffer.edit(Vec::new(), &mut |edit| edit.replace(8, 0, b"!"));
        core.external_edit();
        assert_eq!(core.composition(), Some(4..8));

        buffer.edit(Vec::new(), &mut |edit| edit.replace(0, 0, b"> "));
        core.external_edit();
        assert_eq!(core.composition(), Some(6..10));
        core.execute_command(EditorCommand::Ime(ImeCommand::CommitText("hello")));
        let read = buffer.read().unwrap();
        assert_eq!(
            read.slice(0..read.len()).as_ref(),
            format!("> say hello!{suffix}").as_bytes()
        );
        assert_eq!(core.composition(), None);
    }
}
