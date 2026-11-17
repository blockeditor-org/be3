use super::*;

#[test]
fn a_job_is_answered_with_its_tile() {
    let mut outbound = Vec::new();
    let mut tiles = Hosted::start::<Tiles>(&mut outbound).expect("the rasterizer starts");
    assert!(outbound.is_empty());
    let id = TileId { zoom: 0, x: 0, y: 0 };

    let job = Job {
        requester: 1,
        id,
        source: id,
        body: Vec::new(),
    };
    tiles
        .receive(job.encode(), &mut outbound)
        .expect("the rasterizer takes the job");
    let elsewhere = TileId { zoom: 1, x: 1, y: 0 };
    let not_a_tile = Job {
        requester: 2,
        id: elsewhere,
        source: elsewhere,
        body: vec![0x1f, 0x8b, 0],
    };
    tiles
        .receive(not_a_tile.encode(), &mut outbound)
        .expect("the rasterizer takes the job");

    let [empty, broken] = <[Vec<u8>; 2]>::try_from(outbound).expect("an answer for each job");
    let empty = Rasterized::decode(&empty).expect("the answer decodes");
    assert_eq!((empty.requester, empty.id), (1, id));
    let raster = empty.result.expect("an empty tile is drawn");
    assert_eq!(raster.pixels.len(), TILE_PIXELS * TILE_PIXELS * 4);
    assert!(raster.labels.is_empty());
    let broken = Rasterized::decode(&broken).expect("the answer decodes");
    assert_eq!((broken.requester, broken.id), (2, elsewhere));
    assert!(broken.result.is_err());
}
