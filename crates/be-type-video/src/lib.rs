mod video;

pub use video::{
    ClipNode, DEFAULT_CLIP_SECONDS, MAX_CLIP_LENGTH, Video, VideoAttachment, VideoClip,
    VideoClipTiming, VideoContent, VideoEffect, VideoFrameRate, VideoOperation, VideoProject,
};

be_sdk_type_guest::be_block_types![VideoContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;
