mod file;
mod folder;
mod workspace;

pub use file::{AnyFile, FileContent, FileHeader};
pub use folder::{Folder, FolderContent, FolderEntry};
pub use workspace::{Workspace, WorkspaceContent};

use be_block::plain::media_type_of;
use block_type_guest::BlobFile;

impl BlobFile for AnyFile {
    const ANY_FILE: bool = true;

    fn header(name: &str, _: &[u8]) -> FileHeader {
        FileHeader {
            source_name: name.to_owned(),
            media_type: media_type_of(name).to_owned(),
        }
    }

    fn media_type(header: &FileHeader) -> String {
        header.media_type.clone()
    }

    fn claims(_: &str, _: &[u8]) -> bool {
        true
    }
}

block_type_guest::be_block_types![
    be_block::EditorViewContent,
    FileContent { file },
    FolderContent { directory },
    WorkspaceContent { root },
];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
