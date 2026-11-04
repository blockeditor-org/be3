use super::*;

#[test]
fn a_minute_keeps_the_peak_of_each_measurement_through_it() {
    let mut history = History::default();
    assert_eq!(history.record(sample(600, 300, 40)), None);
    assert_eq!(history.record(sample(610, 500, 20)), None);
    assert_eq!(history.record(sample(650, 400, 60)), None);
    let ended = history.record(sample(660, 100, 10));
    assert_eq!(
        ended,
        Some(Minute {
            point: MemoryPoint {
                at: 600,
                usage: usage(500, 60),
            },
            guests: [("app be3:desktop".to_owned(), 60 << 20)].into(),
        }),
        "a sample in the next minute ends the last, with the peak of each measurement and guest"
    );
    let view = history.view().expect("a sample has been taken");
    assert_eq!(
        view.points
            .iter()
            .map(|point| (point.at, point.usage.resident >> 20))
            .collect::<Vec<_>>(),
        [(600, 500), (660, 100)],
        "the view has the ended minutes and the minute so far"
    );
    assert_eq!(view.now, 660);
    let resident = view
        .rows
        .iter()
        .find(|row| row.row.name == "Resident")
        .expect("a row for resident memory");
    assert_eq!(resident.line, Some(MemoryLine::Resident));
    assert_eq!(
        (
            resident.row.current.trim(),
            resident.row.average.trim(),
            resident.row.peak.trim()
        ),
        ("100.0 MiB", "300.0 MiB", "500.0 MiB"),
        "the latest sample, and the average and peak of the minutes"
    );
    let guest = view
        .rows
        .iter()
        .find(|row| row.row.name == "app be3:desktop")
        .expect("a row for each guest");
    assert_eq!(guest.row.current, bytes(10 << 20));
    assert_eq!(
        guest.row.peak,
        bytes(60 << 20),
        "a guest's peak is kept for the run"
    );
}
