use super::*;

use block_plugin_api::{ContactsWatch, HostContact, Watched};

#[test]
fn the_contacts_reach_only_the_shell() {
    let mut instances = placed();
    let contacts = vec![HostContact {
        account: [7; 16],
        display_name: "Ada".to_owned(),
        email: "ada@example.com".to_owned(),
        verified: true,
    }];
    instances.next_screens(PASS);

    assert!(watch::<ContactsWatch>(&mut instances));
    assert!(
        !publish::<ContactsWatch>(&mut instances, &contacts),
        "only the shell may see the contacts"
    );
    assert!(watched_sent::<ContactsWatch>(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.set_shell(Some(INSTANCE)));
    assert!(instances.watches_published(ContactsWatch::OPEN));
    assert_eq!(
        watched_sent::<ContactsWatch>(&instances.next_screens(PASS).opened),
        vec![contacts.clone()],
        "an instance that becomes the shell is told what it was waiting for"
    );

    instances.set_shell(None);
    assert!(
        !publish::<ContactsWatch>(&mut instances, &Vec::new()),
        "nor does an instance that is no longer the shell"
    );
    assert!(watched_sent::<ContactsWatch>(&instances.next_screens(PASS).opened).is_empty());
}
