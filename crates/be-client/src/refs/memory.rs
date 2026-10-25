use std::{collections::BTreeMap, sync::Mutex};

use be_store::StoreError;

use super::{Row, Scope, Table};

#[derive(Default)]
pub(crate) struct MemoryRefs {
    rows: Mutex<BTreeMap<(Scope, Table, Vec<u8>), Vec<u8>>>,
}

impl MemoryRefs {
    pub(crate) fn load(&self, scope: &Scope) -> Result<Vec<Row>, StoreError> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .iter()
            .filter(|((held, _, _), _)| held == scope)
            .map(|((_, table, key), value)| Row {
                table: *table,
                key: key.clone(),
                value: Some(value.clone()),
            })
            .collect())
    }

    pub(crate) fn write(&self, scope: &Scope, rows: &[Row]) -> Result<(), StoreError> {
        let mut held = self.rows.lock().unwrap();
        for row in rows {
            let key = (scope.clone(), row.table, row.key.clone());
            match &row.value {
                Some(value) => held.insert(key, value.clone()),
                None => held.remove(&key),
            };
        }
        Ok(())
    }
}
