use super::*;

#[test]
fn runs_list_every_fragment_with_what_it_holds() {
    let mut sequence = loaded("Hello world!");
    typed(&mut sequence, BOB, 6, "hi! ");
    let delete = sequence
        .delete(6..10)
        .expect("there is something to delete");
    applied(&mut sequence, &delete);

    let runs: Vec<(Pos, u64, bool, String)> = sequence
        .runs()
        .map(|run| {
            (
                run.first,
                run.len,
                run.visible,
                String::from_utf8_lossy(run.items).into_owned(),
            )
        })
        .collect();
    let at = |client, offset| Pos { client, offset };
    assert_eq!(
        runs,
        vec![
            (at(LOADED, 0), 6, true, "Hello ".to_owned()),
            (at(BOB, 0), 4, false, "hi! ".to_owned()),
            (at(LOADED, 6), 6, true, "world!".to_owned()),
        ]
    );
    assert_eq!(text(&sequence), "Hello world!");
}
