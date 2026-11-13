use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_points_an_old_command_name_at_its_new_form() {
    let document = build_node(|| {
        let name = create_signal(String::new());
        view! {
            <TextInput @test_id={"form.name"} value={name} placeholder="Name" />
        }
    });
    let mut driven = stepped(crate::native::testing::Showing(document));
    for (old, new) in [
        ("gone", "wait not"),
        ("reveal", "find TARGET reveal"),
        ("shown", "find TARGET shown"),
    ] {
        let refused = driven
            .ask(&[old, "#x"])
            .expect_err("an old name is refused");
        assert!(
            refused.contains("is now") && refused.contains(new),
            "{old}: {refused}"
        );
    }
}
