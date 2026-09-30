use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 800.0);

#[test]
fn a_reopened_phone_goes_back_through_the_files_in_the_order_they_were_shown() {
    let (mut fixture, first) = profiled_sized(Some(PHONE), None);
    let second = Uuid::new_v4();
    let third = Uuid::new_v4();
    show(&mut fixture, first, None);
    show(&mut fixture, second, None);
    show(&mut fixture, third, None);
    show(&mut fixture, first, None);
    let layout = profile(&fixture)
        .state("layout")
        .cloned()
        .expect("the layout is saved in the profile");

    let (mut reopened, _) = profiled_sized(Some(PHONE), Some(layout));
    reopened.settle();
    assert_eq!(reopened.shown(), vec![first]);

    reopened.test.click("dock.switch");
    reopened.settle();
    reopened.test.click("dock.switcher.close.2");
    reopened.settle();
    assert_eq!(
        reopened.shown(),
        vec![third],
        "closing the file on show goes back to the one shown before it, \
         not the tab beside it"
    );
}
