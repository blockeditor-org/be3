use super::*;

mod a_login_session_entry_quotes_a_path_with_reserved_characters;
mod installing_adds_the_lock_screens_pam_service_without_replacing_one;
mod installing_asks_root_only_for_what_it_cannot_write;
mod installing_installs_only_what_the_build_produced_and_refuses_a_partial_build;
mod installing_lists_the_login_session_once_where_greetds_greeters_look;
mod installing_replaces_the_line_an_earlier_install_listed_for_greetd;
mod installing_the_login_session_puts_be_host_under_the_prefix_and_its_entry_where_display_managers_look;
mod lightdms_configured_sessions_directory_gets_the_entry_too;
mod the_login_session_entry_runs_the_installed_desktop_on_the_displays;
mod uninstalling_removes_exactly_what_installing_placed;

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("be-host-{name}-{}", uuid::Uuid::new_v4()))
}

fn build(source: &Path) {
    std::fs::create_dir_all(source.join("data/notes/files")).unwrap();
    std::fs::write(source.join(EXECUTABLE), "be-host").unwrap();
    std::fs::write(source.join("apps.json"), "[\n  \"notes.app.json\"\n]\n").unwrap();
    std::fs::write(source.join("notes.app.json"), "{}").unwrap();
    std::fs::write(source.join("data/notes/files/readme.txt"), "hello").unwrap();
    std::fs::write(
        source.join(LIST),
        "apps.json\nbe-host\ndata/notes/files/readme.txt\nnotes.app.json\n",
    )
    .unwrap();
}

fn layout(root: &Path) -> Layout {
    Layout {
        prefix: root.join("prefix"),
        sessions: vec![
            root.join("usr/share/wayland-sessions"),
            root.join("lightdm"),
        ],
        pam: root.join("etc/pam.d"),
        greetd: root.join("etc/greetd/environments"),
    }
}

fn install_into(source: &Path, layout: &Layout) -> Receipt {
    let (changes, receipt) = plan_install(source, layout).unwrap();
    apply_here(&changes).unwrap();
    receipt
}
