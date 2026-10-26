use be_model::Model;

use crate::database::Database;
use crate::database_schema::DatabaseSchema;
use crate::database_view::DatabaseView;

#[test]
fn databases_read_every_register_without_any_one_field() {
    for kind in [
        Database::kind(),
        DatabaseSchema::kind(),
        DatabaseView::kind(),
    ] {
        assert_eq!(kind.registers_accept_missing_fields(), Vec::<String>::new());
    }
}
