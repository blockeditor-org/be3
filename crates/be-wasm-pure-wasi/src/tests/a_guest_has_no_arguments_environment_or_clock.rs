use super::*;

#[test]
fn a_guest_has_no_arguments_environment_or_clock() {
    let mut wasi = Wasi::new([0; 32]);
    for sizes in ["args_sizes_get", "environ_sizes_get"] {
        let mut memory = Bytes(vec![0xff; 8]);
        let answered = wasi.call(sizes, &[0, 4], &mut memory, &mut |_| {});
        assert_eq!((answered, memory.0), (Ok(SUCCESS), vec![0; 8]), "{sizes}");
    }
    for clock in ["clock_time_get", "clock_res_get"] {
        let answered = wasi.call(clock, &[1, 0, 0], &mut Bytes(vec![0; 8]), &mut |_| {});
        assert_eq!(answered, Ok(ENOSYS), "{clock}");
    }
    for name in ANSWERED {
        assert!(parameters(name).is_some(), "{name} has no signature");
    }
}
