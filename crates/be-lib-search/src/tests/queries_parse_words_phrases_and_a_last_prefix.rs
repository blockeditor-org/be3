use crate::{Clause, Query};

#[test]
fn queries_parse_words_phrases_and_a_last_prefix() {
    let parse = |text: &str| Query::parse(text).clauses;
    let word = |word: &str| Clause::Word(word.into());
    assert_eq!(parse("Hello wor"), [word("hello"), Clause::Prefix("wor".into())]);
    assert_eq!(parse("hello world "), [word("hello"), word("world")]);
    assert_eq!(parse("hello w"), [word("hello"), word("w")]);
    assert_eq!(
        parse("\"Big River\" stone"),
        [Clause::Phrase(vec!["big".into(), "river".into()]), Clause::Prefix("stone".into())]
    );
    assert_eq!(parse("say \"one\""), [word("say"), word("one")]);
    assert_eq!(parse("unclosed \"quote"), [word("unclosed"), Clause::Prefix("quote".into())]);
    assert_eq!(parse("STRASSE straße"), [word("strasse"), Clause::Prefix("strasse".into())]);
    assert!(parse(" , ").is_empty());
}
