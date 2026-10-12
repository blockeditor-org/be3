use super::*;
use crate::plugin_host::HostChildStatus;
use block_plugin_api::ChildId;

fn statuses_sent(messages: &[Message]) -> Vec<Vec<ChildId>> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::ChildStatuses(statuses) => {
                Some(statuses.iter().map(|status| status.child).collect())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_restarted_plugin_hears_its_childrens_statuses_again() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let status = HostChildStatus {
        child: ChildId(7),
        available: true,
        intrinsic: None,
        aspect_ratio: None,
        active: false,
        interaction: Default::default(),
        capabilities: Default::default(),
        resize: Default::default(),
        error: None,
        menu: Vec::new(),
        creation: None,
        settings: None,
    };
    assert_eq!(
        statuses_sent(&instances.set_child_statuses(INSTANCE, REGION, vec![status.clone()])),
        vec![vec![ChildId(7)]]
    );
    assert!(
        instances
            .set_child_statuses(INSTANCE, REGION, vec![status.clone()])
            .is_empty(),
        "an unchanged status is not news"
    );

    instances.reopen();

    assert_eq!(
        statuses_sent(&instances.next_screens(PASS).opened),
        vec![vec![ChildId(7)]],
        "a restarted plugin is told again"
    );
}
