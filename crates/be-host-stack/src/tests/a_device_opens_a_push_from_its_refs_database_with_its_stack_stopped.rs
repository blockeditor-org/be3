use be_host_base::host_state::{HostStateStore, SavedAccount, ServerLocation};

use super::*;
use crate::device_push::{self, Shown};

/// A phone that keeps its refs database and host state in a directory, as be-host does, and
/// takes pushes at a Web Push endpoint with keys of its own (Android's UnifiedPush receiver),
/// is told of a message while signed out and with no stack running: the body the server pushed
/// opens from the directory alone, named from the phone's own contacts and graph. A push shown
/// once, sent again under another encryption, or not decrypting is dropped; one the phone
/// cannot open shows "New message" naming nothing. Turning pushes off leaves nothing to
/// register.
#[test]
fn a_device_opens_a_push_from_its_refs_database_with_its_stack_stopped() {
    let harness = Harness::pushing();
    let pushes = PushService::start();
    let personal = harness.connect_personal();
    let phone_dir = harness.directory.join("phone");
    let phone = Member::join_keeping(
        &harness,
        personal,
        "dev@example.com",
        "Dev",
        crate::refs_in(&phone_dir).expect("the phone's refs database opens"),
    );
    let block = Uuid::new_v4();
    let chat = be_type_chat::ChatContent::default();
    create(
        block,
        be_type_chat::ChatContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Lunch"),
        Some(chat.encode()),
        false,
    );
    flush();
    phone.may_view(block);
    phone.reads(block);
    phone
        .runtime
        .block_on(phone.peer.refresh_contacts())
        .expect("the phone lists its contacts");

    // The phone's host state: its account, as signing in saves it, and the keys its endpoint
    // was registered with.
    let state = HostStateStore::open(phone_dir.join("host.sqlite3")).expect("the state opens");
    let saved = SavedAccount {
        server: ServerLocation::Remote(harness.url.clone()),
        id: phone.account(),
        email: "dev@example.com".to_owned(),
        name: "Dev".to_owned(),
        token: phone.someone.session.token.clone(),
        last_organization_id: Some(personal),
        authorization: None,
    };
    state.save_account(&saved).expect("the account is saved");
    state
        .set_identity(&saved, phone.someone.phrase.secret().to_bytes())
        .expect("the identity is saved");
    let keys = device_push::keys(&state).expect("the phone makes its push keys");
    assert_eq!(device_push::keys(&state).expect("kept"), keys);
    let address = format!("{}/phone", pushes.url);
    state
        .set_push_endpoint(Some(&address))
        .expect("the distributor's endpoint is kept");
    let registration = device_push::registration(&state)
        .expect("the registration reads")
        .expect("pushes are on");
    assert_eq!(
        (
            &registration.address,
            &registration.p256dh,
            &registration.auth
        ),
        (&address, &keys.public(), &keys.auth().to_vec())
    );
    phone
        .runtime
        .block_on(phone.peer.register_endpoint(registration))
        .expect("the phone registers its endpoint");
    phone.signs_out();

    // The sender's connection lives on the runtime it was made on.
    let sending = runtime();
    let sender = sending.block_on(outside_peer(
        harness.outside_config().organization(Some(personal)),
    ));
    sending
        .block_on(sender.notify(block, "Noon at the usual place", Vec::new(), Vec::new()))
        .expect("the notification is sent");
    let pushed = pushes.next();
    assert_eq!(pushed.to, "phone");
    stop_and_wait();
    assert_eq!(crate::running_organization(), None);

    let now = be_block_commit::now_milliseconds();
    assert_eq!(
        device_push::receive(&state, &phone_dir, &pushed.body, now),
        Some(Shown {
            title: "Lunch".to_owned(),
            body: "Counter: Noon at the usual place".to_owned(),
            opens: Some((personal, block)),
        })
    );
    // The same push again, and the same message resent under a new encryption, are dropped.
    assert_eq!(
        device_push::receive(&state, &phone_dir, &pushed.body, now),
        None
    );
    let plain = keys.open(&pushed.body).expect("the push opens");
    let resent = be_lib_web_push::encrypt(&keys.public(), &keys.auth(), &plain, 0)
        .expect("the push encrypts again");
    assert_eq!(device_push::receive(&state, &phone_dir, &resent, now), None);

    // What does not decrypt is dropped, as a browser drops it; a push this device holds no
    // account for names nothing: another device's state, with keys of its own and no account
    // saved.
    let mut garbled = pushed.body.clone();
    *garbled.last_mut().expect("a push has a body") ^= 1;
    assert_eq!(
        device_push::receive(&state, &phone_dir, &garbled, now),
        None
    );
    let stranger_dir = harness.directory.join("stranger");
    std::fs::create_dir_all(&stranger_dir).expect("the stranger's directory is made");
    let stranger =
        HostStateStore::open(stranger_dir.join("host.sqlite3")).expect("the state opens");
    let theirs = device_push::keys(&stranger).expect("the stranger makes its push keys");
    let elsewhere = be_lib_web_push::encrypt(&theirs.public(), &theirs.auth(), &plain, 0)
        .expect("the push encrypts to the stranger");
    assert_eq!(
        device_push::receive(&stranger, &stranger_dir, &elsewhere, now),
        Some(Shown::generic())
    );

    // Turned off, the endpoint is forgotten and handed back for the server to forget: no
    // organization registers it again.
    assert_eq!(
        device_push::turn_off(&state).expect("pushes turn off"),
        Some(address)
    );
    assert_eq!(device_push::registration(&state).expect("it reads"), None);
}
