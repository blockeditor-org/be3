use super::*;

#[test]
fn a_job_is_answered_with_its_tile() {
    let mut outbound = Vec::new();
    let mut tiles = Hosted::start::<Tiles>(&mut outbound).expect("the rasterizer starts");
    assert!(outbound.is_empty());
    let id = TileId { zoom: 0, x: 0, y: 0 };
    let elsewhere = TileId { zoom: 1, x: 1, y: 0 };

    let job = Job {
        id,
        source: id,
        body: Vec::new(),
    };
    let empty = tiles.ask(&job.encode()).expect("the rasterizer takes the job");
    let not_a_tile = Job {
        id: elsewhere,
        source: elsewhere,
        body: vec![0x1f, 0x8b, 0],
    };
    let broken = tiles
        .ask(&not_a_tile.encode())
        .expect("the rasterizer takes the job");

    let [empty] = <[Vec<u8>; 1]>::try_from(empty).expect("one answer for the job");
    let empty = Rasterized::decode(&empty).expect("the answer decodes");
    assert_eq!(empty.id, id);
    let raster = empty.result.expect("an empty tile is drawn");
    assert_eq!(raster.pixels.len(), TILE_PIXELS * TILE_PIXELS * 4);
    assert!(raster.labels.is_empty());
    let [broken] = <[Vec<u8>; 1]>::try_from(broken).expect("one answer for the job");
    let broken = Rasterized::decode(&broken).expect("the answer decodes");
    assert_eq!(broken.id, elsewhere);
    assert!(broken.result.is_err());
}
