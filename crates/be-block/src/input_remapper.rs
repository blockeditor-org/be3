use be_model::{Bytes, Document, Edit, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct InputRemapper {
    pub plugin: Option<String>,
    pub module: Option<String>,
    pub config: Option<Bytes>,
}

impl InputRemapper {
    pub fn set(plugin: &str, module: &str, config: Option<Vec<u8>>) -> Edit {
        [
            Self::PLUGIN.set(ObjectId::ROOT, &Some(plugin.to_owned())),
            Self::MODULE.set(ObjectId::ROOT, &Some(module.to_owned())),
            Self::CONFIG.set(ObjectId::ROOT, &config.map(Bytes)),
        ]
        .into_iter()
        .collect()
    }

    pub fn runs(&self) -> Option<(&str, &str, &[u8])> {
        Some((
            self.plugin.as_deref()?,
            self.module.as_deref()?,
            self.config.as_deref()?.as_slice(),
        ))
    }
}

impl Root for InputRemapper {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696e_7075_742d_7265_6d61_702d_626c_6f31);
}

pub type InputRemapperContent = Document<InputRemapper>;
