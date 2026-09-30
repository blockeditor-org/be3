use super::*;

#[test]
fn zooming_in_draws_deeper_tiles_over_the_shallower_ones() {
    let mut editor = editor();
    serve_tiles(&mut editor);

    for _ in 0..5 {
        editor.click("map.zoom-in");
        editor.run();
    }

    let deeper = requested_tiles(&mut editor);
    assert!(!deeper.is_empty());
    assert!(
        deeper
            .iter()
            .all(|(_, url)| url.contains("/shortbread_v1/3/"))
    );
    assert!(
        deeper
            .iter()
            .all(|(_, url)| tile_drawn(&editor, &tile_test_id(url)))
    );
    for (request, _) in &deeper {
        editor.reply(
            *request,
            HostReply::Fetched(FetchResult::Body(ocean_tile())),
        );
    }
    let rest = serve_tiles(&mut editor);

    assert!(rest.iter().all(|url| url.contains("/shortbread_v1/3/")));
}
