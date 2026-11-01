use super::*;

#[test]
fn a_scan_that_could_not_run_lists_no_programs_and_says_why() {
    let programs = Programs::<()>::default();
    programs.scan_on(WAITING_ICON_SIZE, |_| {
        Err(std::io::Error::other("no threads left"))
    });
    assert!(
        programs.scanned(),
        "a failed scan left the programs loading"
    );
    assert!(programs.listed().is_empty());
    assert!(
        block_app_base::notices::shown()
            .iter()
            .any(|notice| notice.message == "Programs could not be listed: no threads left"),
        "the failure was not reported"
    );

    let programs = Programs::<()>::default();
    programs.scan_on(WAITING_ICON_SIZE, |job| {
        drop(job);
        Ok(())
    });
    assert!(
        !programs.scanned(),
        "the stopped scan was taken in before the frame delivered it"
    );
    block_app_base::host::deliver();
    assert!(
        programs.scanned(),
        "a scan that stopped left the programs loading"
    );
    assert!(
        block_app_base::notices::shown()
            .iter()
            .any(|notice| notice.message == "Programs could not be listed: the scan stopped"),
        "the stopped scan was not reported"
    );
}
