use super::*;

#[test]
fn every_type_keeps_everything_inside_the_organizations_history() {
    let mut organization = OrganizationPolicies::default();
    organization.history_days = Some(30);
    let now = 100 * DAY;
    let summaries: Vec<CommitSummary> = (0..20u8)
        .map(|step| CommitSummary {
            id: commit(step),
            time: now - i64::from(step) * DAY - i64::from(step),
            pinned: false,
        })
        .collect();
    for content_type in [SEARCH_INDEX_TYPE, Uuid::nil()] {
        let plan = plan_retention(&summaries, now, &policy(content_type, &organization));
        assert!(
            plan.drop.is_empty(),
            "a block of {content_type} dropped states inside the organization's thirty days: {plan:?}"
        );
    }
    let short = plan_retention(
        &summaries,
        now,
        &policy(SEARCH_INDEX_TYPE, &OrganizationPolicies::default()),
    );
    assert_eq!(short.keep, vec![commit(0)], "{short:?}");
}
