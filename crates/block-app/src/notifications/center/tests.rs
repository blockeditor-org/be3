use super::*;

mod a_new_notification_gets_a_fresh_id_and_a_replacement_keeps_its_own;
mod a_toast_hides_when_its_time_is_up_and_the_notification_stays_listed;
mod closing_a_notification_reports_why;
mod invoking_an_action_reports_it_and_closes_the_notification_unless_it_is_resident;
mod the_oldest_notifications_are_let_go_once_too_many_are_kept;

fn incoming(summary: &str) -> Incoming {
    Incoming {
        app_name: "Mail".to_owned(),
        summary: summary.to_owned(),
        expire_timeout: -1,
        ..Incoming::default()
    }
}

fn with_actions(summary: &str, keys: &[&str]) -> Incoming {
    Incoming {
        actions: keys
            .iter()
            .map(|key| Action {
                key: (*key).to_owned(),
                label: key.to_uppercase(),
            })
            .collect(),
        ..incoming(summary)
    }
}

fn seconds(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

fn toasted(center: &Center) -> Vec<u32> {
    center.toasts().map(|kept| kept.id).collect()
}

fn listed(center: &Center) -> Vec<u32> {
    center.listed().map(|kept| kept.id).collect()
}
