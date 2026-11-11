use crate::{
    Owner,
    search::{SearchAsk, Searching},
};

#[test]
fn an_empty_search_request_is_not_a_change() {
    let mut search = Searching::default();
    for _ in 0..3 {
        assert!(
            !search.want(Owner::Host, Vec::new()),
            "nothing asked, nothing before"
        );
    }
    let ask = vec![SearchAsk {
        text: "word".into(),
        within: None,
    }];
    assert!(
        search.want(Owner::Host, ask.clone()),
        "a new ask is a change"
    );
    assert!(!search.want(Owner::Host, ask), "the same ask is not");
    assert!(
        search.want(Owner::Host, Vec::new()),
        "withdrawing is a change"
    );
    assert!(!search.want(Owner::Host, Vec::new()), "and only once");
}
