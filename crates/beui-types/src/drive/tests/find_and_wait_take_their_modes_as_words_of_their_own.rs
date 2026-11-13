use super::*;

#[test]
fn find_and_wait_take_their_modes_as_words_of_their_own() {
    let parse = |line: &str| {
        let mut words = line.split(' ').map(str::to_owned);
        let command = words.next().unwrap();
        parse(&command, &words.collect::<Vec<_>>()).expect("a reading command")
    };
    assert_eq!(parse("find #row"), Ok(Command::Find("#row".into())));
    assert_eq!(
        parse("find #row reveal"),
        Ok(Command::Reveal("#row".into()))
    );
    assert_eq!(parse("find #row shown"), Ok(Command::Shown("#row".into())));
    assert_eq!(
        parse("find reveal"),
        Ok(Command::Find("reveal".into())),
        "a mode alone is the target"
    );
    assert!(parse("find").is_err());
    assert_eq!(
        parse("wait not #row over #list"),
        Ok(Command::Wait {
            text: "#row".into(),
            over: Some("#list".into()),
            present: false,
        })
    );
    assert!(parse("wait not").is_err());
    for old in ["gone #row", "reveal #row", "shown #row"] {
        let refused = parse(old).expect_err("an old name is refused");
        assert!(refused.contains("is now"), "{refused}");
    }
}
