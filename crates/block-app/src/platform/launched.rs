use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use jni::EnvUnowned;
use jni::objects::{JClass, JString};
use jni::refs::Reference;

pub(crate) struct Launched {
    build: PathBuf,
    data: PathBuf,
}

impl Launched {
    pub(crate) fn assets(&self) -> PathBuf {
        self.build.join("assets")
    }

    pub(crate) fn data(&self) -> &Path {
        &self.data
    }
}

static LAUNCHED: OnceLock<Launched> = OnceLock::new();

pub(crate) fn launched() -> Option<&'static Launched> {
    LAUNCHED.get()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_MainActivity_nativeLaunched(
    _env: EnvUnowned<'_>,
    _: JClass<'_>,
    build: JString<'_>,
    data: JString<'_>,
) {
    if build.is_null() || data.is_null() {
        return;
    }
    let _ = LAUNCHED.set(Launched {
        build: PathBuf::from(build.to_string()),
        data: PathBuf::from(data.to_string()),
    });
}
