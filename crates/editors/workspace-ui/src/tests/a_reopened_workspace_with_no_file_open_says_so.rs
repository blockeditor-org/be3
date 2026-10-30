use super::*;

#[test]
fn a_reopened_workspace_with_no_file_open_says_so() {
    let (fixture, _) = profiled_sized(Some(WIDE), None);
    assert!(fixture.says("No file open"));

    let layout = profile(&fixture)
        .state("layout")
        .cloned()
        .expect("the layout is saved in the profile");
    let (mut reopened, _) = profiled_sized(Some(WIDE), Some(layout));
    reopened.settle();

    assert!(
        reopened.says("No file open"),
        "the blank editors pane says so when the workspace reopens with its saved layout"
    );
}
