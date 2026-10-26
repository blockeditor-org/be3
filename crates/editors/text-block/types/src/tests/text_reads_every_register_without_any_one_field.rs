use be_model::Model;

use crate::TextBlock;

#[test]
fn text_reads_every_register_without_any_one_field() {
    assert_eq!(
        TextBlock::kind().registers_accept_missing_fields(),
        Vec::<String>::new()
    );
}
