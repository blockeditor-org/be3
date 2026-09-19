use super::*;

#[test]
fn children_written_as_a_prop_and_between_the_tags_are_rejected() {
    let Err(error) = syn::parse2::<View>(quote! {
        <List spacing=4.0 children>
            <Text string="one" />
        </List>
    }) else {
        panic!("children written both ways are rejected");
    };

    assert_eq!(
        error.to_string(),
        "`children` is set both as a prop and as this tag's children, write it one way or the other"
    );
}
