use std::sync::mpsc::{Sender, channel};

use block_plugin_api::IncomingNotification;

use super::*;

mod a_replacement_keeps_its_id_while_the_desktop_holds_it;
mod requests_wait_for_the_desktop_until_it_says_it_has_them;

type Harness = (
    Inbox,
    Arc<Mutex<Ids>>,
    Sender<NotificationRequest>,
    Receiver<NotificationRequest>,
);

fn inbox() -> Harness {
    let ids = Arc::new(Mutex::new(Ids::default()));
    let (sender, receiver) = channel();
    (Inbox::new(Arc::clone(&ids)), ids, sender, receiver)
}

fn notify(ids: &Mutex<Ids>, sender: &Sender<NotificationRequest>, replaces: u32) -> u32 {
    let mut ids = ids.lock().unwrap();
    let id = ids.assign(replaces);
    sender
        .send(NotificationRequest::Notify(Box::new(
            IncomingNotification {
                id,
                summary: format!("number {id}"),
                ..IncomingNotification::default()
            },
        )))
        .unwrap();
    id
}

fn sequences(inbox: &Inbox) -> Vec<u64> {
    inbox
        .published()
        .requests
        .iter()
        .map(|(sequence, _)| *sequence)
        .collect()
}
