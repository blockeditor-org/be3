use super::*;

#[test]
fn what_was_read_is_read_again_once_a_notice_names_it() {
    let (named, other, session) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut cache = Cache {
        owned: Some(vec![named]),
        ..Cache::default()
    };
    cache.settings.insert(named, Settings::default());
    cache.settings.insert(other, Settings::default());
    cache.editors.insert(session, Uuid::new_v4());

    cache.heard(&be_host_stack::Changes {
        blocks: [named].into_iter().collect(),
        ..be_host_stack::Changes::default()
    });
    assert!(!cache.settings.contains_key(&named), "a changed block is read again");
    assert!(cache.settings.contains_key(&other), "the others are kept");
    assert!(cache.owned.is_some() && cache.editors.contains_key(&session));

    cache.heard(&be_host_stack::Changes {
        graph: true,
        ..be_host_stack::Changes::default()
    });
    assert!(cache.owned.is_none(), "the roots are read again from a changed graph");
    assert!(
        cache.editors.is_empty(),
        "sessions' editors are read again from a changed graph"
    );

    cache.heard(&be_host_stack::Changes::everything());
    assert!(cache.settings.is_empty(), "a restarted stack reads every block again");
}
