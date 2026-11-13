use super::*;
use uuid::Uuid;

mod a_blob_saves_its_header_and_payload_as_separate_parts;
mod a_block_link_is_found_by_its_fragment_whatever_its_host;
mod a_deleted_child_of_a_view_is_blanked_in_the_state_that_referenced_it;
mod a_quoted_name_cannot_fake_what_a_dialog_says;
mod an_icon_saved_as_a_material_codepoint_reads_as_its_lucide_name;
mod block_metadata_keeps_keys_it_does_not_know_and_reads_each_key_alone;
mod block_urls_include_organization_and_reject_malformed_paths;
mod defined_metadata_merges_key_by_key;
mod icon_overrides_round_trip_and_merge_independently;
mod metadata_that_points_to_a_block_lists_it_as_a_reference;
mod streamed_content_separates_its_header_from_its_payload;

#[derive(Clone, Debug, Default, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
struct Header {
    source_name: String,
    media_type: String,
}

struct Bytes;

impl crate::blob::BlobKind for Bytes {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6279_7465_732d_626c_6f63_6b2d_7465_7374);

    type Header = Header;

    fn name(header: &Header) -> &str {
        &header.source_name
    }
}

type BytesContent = crate::blob::Blob<Bytes>;

fn header(name: &str) -> Header {
    Header {
        source_name: name.into(),
        media_type: "image/png".into(),
    }
}

fn image(name: &str, payload: &[u8]) -> BytesContent {
    BytesContent::new(header(name), payload.to_vec())
}
