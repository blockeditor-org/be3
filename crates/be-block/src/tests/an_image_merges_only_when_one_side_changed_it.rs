use super::*;

#[test]
fn an_image_merges_only_when_one_side_changed_it() {
    let base = image("photo.png", b"original pixels");
    let ours = image("photo.png", b"our pixels");
    let theirs = image("photo.png", b"their pixels");

    assert_eq!(
        ImageContent::merge3(&base, &ours, &base),
        MergeResult::Clean(ours.clone())
    );
    assert_eq!(
        ImageContent::merge3(&base, &base, &theirs),
        MergeResult::Clean(theirs.clone())
    );
    assert_eq!(
        ImageContent::merge3(&base, &ours, &ours),
        MergeResult::Clean(ours.clone())
    );

    let contested = ImageContent::merge3(&base, &ours, &theirs);
    assert!(!contested.is_clean(), "two rewrites merged silently");
    assert_eq!(
        contested,
        MergeResult::Conflicted {
            value: ours.clone(),
            conflicts: 1
        }
    );
    assert_eq!(contested.value(), ours);
    assert_eq!(ImageContent::CONTENT_TYPE, ImageContent::CONTENT_TYPE);
    assert_eq!(base.name().as_deref(), Some("photo.png"));
    assert!(base.references().is_empty());
}
