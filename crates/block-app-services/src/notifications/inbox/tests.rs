use block_plugin_api::IncomingNotification;

use super::*;

mod a_replacement_keeps_its_id_while_the_desktop_holds_it;
mod requests_wait_for_the_desktop_until_it_says_it_has_them;

fn inbox() -> (Inbox, Arc<Mutex<Ids>>, Vec<NotificationRequest>) {
    let ids = Arc::new(Mutex::new(Ids::default()));
    (Inbox::new(Arc::clone(&ids)), ids, Vec::new())
}

fn notify(ids: &Mutex<Ids>, sent: &mut Vec<NotificationRequest>, replaces: u32) -> u32 {
    let mut ids = ids.lock().unwrap();
    let id = ids.assign(replaces);
    sent.push(NotificationRequest::Notify(IncomingNotification {
        id,
        summary: format!("number {id}"),
        ..IncomingNotification::default()
    }));
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
