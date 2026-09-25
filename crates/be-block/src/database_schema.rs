use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum DatabaseFieldType {
    #[default]
    String,
    Number,
    Enum,
    Block,
    Boolean,
    Color,
    Datetime,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum DatabaseNumberScale {
    #[default]
    Linear,
    Logarithmic,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DatabaseNumberOptions {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub step: Option<f64>,
    pub scale: DatabaseNumberScale,
}

impl DatabaseNumberOptions {
    pub fn effective_step(self) -> f64 {
        self.step.unwrap_or(match self.scale {
            DatabaseNumberScale::Linear => 1.0,
            DatabaseNumberScale::Logarithmic => 1.01,
        })
    }

    pub fn normalized(mut self) -> Self {
        self.minimum = self.minimum.filter(|value| value.is_finite());
        self.maximum = self.maximum.filter(|value| value.is_finite());
        self.step = self.step.filter(|value| value.is_finite());
        if self.scale == DatabaseNumberScale::Logarithmic {
            self.minimum = self.minimum.filter(|value| *value > 0.0);
            self.maximum = self.maximum.filter(|value| *value > 0.0);
        }
        if let (Some(minimum), Some(maximum)) = (self.minimum, self.maximum)
            && minimum > maximum
        {
            self.minimum = Some(maximum);
            self.maximum = Some(minimum);
        }
        self.step = match self.scale {
            DatabaseNumberScale::Linear => self.step.filter(|value| *value > 0.0),
            DatabaseNumberScale::Logarithmic => self.step.filter(|value| *value > 1.0),
        };
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct DatabaseBlockOptions {
    pub block_type: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseEnumOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseField {
    pub id: Uuid,
    pub name: String,
    pub field_type: DatabaseFieldType,
    pub enum_options: Vec<DatabaseEnumOption>,
    pub number_options: DatabaseNumberOptions,
    pub block_options: DatabaseBlockOptions,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DatabaseSchema {
    pub fields: List<SchemaField>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct SchemaField {
    pub name: String,
    pub field_type: DatabaseFieldType,
    pub enum_options: List<EnumOption>,
    pub number_options: DatabaseNumberOptions,
    pub block_options: DatabaseBlockOptions,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct EnumOption {
    pub name: String,
}

impl DatabaseSchema {
    pub fn fields(&self) -> Vec<DatabaseField> {
        self.fields
            .iter()
            .map(|field| DatabaseField {
                id: field.id.as_uuid(),
                name: field.name.clone(),
                field_type: field.field_type,
                enum_options: field
                    .enum_options
                    .iter()
                    .map(|option| DatabaseEnumOption {
                        id: option.id.as_uuid(),
                        name: option.name.clone(),
                    })
                    .collect(),
                number_options: field.number_options,
                block_options: field.block_options,
            })
            .collect()
    }

    pub fn add_field(name: impl Into<String>, field_type: DatabaseFieldType) -> (Uuid, Edit) {
        let field = SchemaField {
            name: name.into(),
            field_type,
            ..SchemaField::default()
        };
        let (id, change) = Self::FIELDS.insert(ObjectId::ROOT, Anchor::End, &field);
        (id.as_uuid(), change.into())
    }

    pub fn remove_field(field: Uuid) -> Edit {
        Change::remove(ObjectId::from_uuid(field)).into()
    }

    pub fn rename_field(field: Uuid, name: impl Into<String>) -> Edit {
        SchemaField::NAME
            .set(ObjectId::from_uuid(field), &name.into())
            .into()
    }

    pub fn set_field_type(field: Uuid, field_type: DatabaseFieldType) -> Edit {
        SchemaField::FIELD_TYPE
            .set(ObjectId::from_uuid(field), &field_type)
            .into()
    }

    pub fn set_number_options(field: Uuid, options: DatabaseNumberOptions) -> Edit {
        SchemaField::NUMBER_OPTIONS
            .set(ObjectId::from_uuid(field), &options.normalized())
            .into()
    }

    pub fn set_block_options(field: Uuid, options: DatabaseBlockOptions) -> Edit {
        SchemaField::BLOCK_OPTIONS
            .set(ObjectId::from_uuid(field), &options)
            .into()
    }

    pub fn add_enum_option(field: Uuid, name: impl Into<String>) -> (Uuid, Edit) {
        let option = EnumOption { name: name.into() };
        let (id, change) =
            SchemaField::ENUM_OPTIONS.insert(ObjectId::from_uuid(field), Anchor::End, &option);
        (id.as_uuid(), change.into())
    }

    pub fn rename_enum_option(option: Uuid, name: impl Into<String>) -> Edit {
        EnumOption::NAME
            .set(ObjectId::from_uuid(option), &name.into())
            .into()
    }

    pub fn remove_enum_option(option: Uuid) -> Edit {
        Change::remove(ObjectId::from_uuid(option)).into()
    }
}

impl Root for DatabaseSchema {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6461_7461_6261_7365_2d73_6368_656d_6101);
}

pub type DatabaseSchemaContent = Document<DatabaseSchema>;
