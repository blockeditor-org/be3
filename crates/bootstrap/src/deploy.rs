use std::process::ExitCode;

use crate::{Result, buck, repository};

pub fn main(arguments: Vec<String>) -> Result<ExitCode> {
    let mut android_id = "com.be3.block".to_owned();
    let mut android_label = "Block".to_owned();
    let mut downloads = true;
    let mut android = true;
    let mut passed = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--android-id" | "--android-label" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("deploy: {argument} needs a value"))?;
                if argument == "--android-id" {
                    android_id = value;
                } else {
                    android_label = value;
                }
                continue;
            }
            "--skip-android" => {
                android = false;
                continue;
            }
            "--skip-downloads" => downloads = false,
            _ => {}
        }
        passed.push(argument);
    }
    if !is_application_id(&android_id) {
        return Err(format!(
            "deploy: --android-id takes an Android application id such as com.be3.block, not '{android_id}'"
        ));
    }
    // unsafe(env)
    unsafe {
        std::env::set_var("BE3_APP_ID", "Block");
        std::env::set_var("BE3_DEPLOY_DEFAULT_SERVER", "true");
    }
    let mut command = vec![
        "run".to_owned(),
        "-m".to_owned(),
        "release".to_owned(),
        "-c".to_owned(),
        "be3.lto=true".to_owned(),
        "-c".to_owned(),
        format!("be3.deploy_downloads={downloads}"),
        "-c".to_owned(),
        format!("be3.deploy_android={android}"),
        "-c".to_owned(),
        format!("be3.android_id={android_id}"),
        "-c".to_owned(),
        format!("be3.android_label={android_label}"),
        "//crates/local-tools:deploy".to_owned(),
        "--".to_owned(),
        "--repository".to_owned(),
        repository().to_string_lossy().into_owned(),
    ];
    command.extend(passed);
    buck::main(command)
}

fn is_application_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|segment| {
            let mut characters = segment.chars();
            characters
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic())
                && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
}
