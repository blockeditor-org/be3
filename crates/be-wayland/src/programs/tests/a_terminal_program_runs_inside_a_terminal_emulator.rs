use super::*;

#[test]
fn a_terminal_program_runs_inside_a_terminal_emulator() {
    let scratch = Scratch::new();
    let top = entry("htop --tree", "Terminal=true");
    let mut environment = environment(&scratch, &[]);
    assert_eq!(environment.command(&top), Err(ExecError::NoTerminal));

    scratch.executable("bin/xterm");
    assert_eq!(
        environment.command(&top).expect("xterm is found"),
        ["xterm", "-e", "htop", "--tree"]
    );

    scratch.executable("bin/foot");
    assert_eq!(
        environment.command(&top).expect("foot is found"),
        ["foot", "htop", "--tree"],
        "foot is preferred, and takes the command as it is"
    );

    environment.terminal = Some("xterm".to_owned());
    assert_eq!(
        environment.command(&top).expect("$TERMINAL is found"),
        ["xterm", "-e", "htop", "--tree"],
        "$TERMINAL wins"
    );

    assert_eq!(
        environment
            .command(&entry("gedit %F", ""))
            .expect("the line expands"),
        ["gedit"],
        "a program that is not a terminal one runs directly"
    );
}
