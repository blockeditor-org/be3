use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::Command,
};

use zip::{ZipWriter, write::SimpleFileOptions};

#[test]
fn reference_verifies() {
    let directory = std::env::temp_dir().join(format!("apk-reference-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let scratch = super::super::Scratch(directory);
    let template = scratch.0.join("template.apk");
    let metadata = scratch.0.join("build-info.json");
    let password = scratch.0.join("password");
    fs::write(&metadata, b"{\"id\":\"example\"}").unwrap();
    fs::write(&password, b"test123").unwrap();
    let manifest = scratch.0.join("AndroidManifest.xml");
    let linked = scratch.0.join("manifest.apk");
    fs::write(
        &manifest,
        br#"<manifest package="com.example.signing"><application/></manifest>"#,
    )
    .unwrap();
    let link = Command::new(Path::new(&std::env::var("APK_SIGN_TOOLS").unwrap()).join("aapt2"))
        .args(["link", "-o"])
        .arg(&linked)
        .arg("--manifest")
        .arg(&manifest)
        .output()
        .unwrap();
    assert!(
        link.status.success(),
        "{}",
        String::from_utf8_lossy(&link.stderr)
    );
    let mut source = zip::ZipArchive::new(fs::File::open(&linked).unwrap()).unwrap();
    let mut binary_manifest = Vec::new();
    source
        .by_name("AndroidManifest.xml")
        .unwrap()
        .read_to_end(&mut binary_manifest)
        .unwrap();
    let mut writer = ZipWriter::new(fs::File::create(&template).unwrap());
    writer
        .start_file(
            "lib/arm64-v8a/libexample.so",
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .with_alignment(16384),
        )
        .unwrap();
    writer.write_all(b"native library").unwrap();
    writer
        .start_file("AndroidManifest.xml", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&binary_manifest).unwrap();
    writer.finish().unwrap();
    for fixture in ["test.jks", "test.p12", "test-keytool.p12"] {
        let key = Path::new(&std::env::var("APK_SIGN_KEYS").unwrap())
            .join("testdata/keystore")
            .join(fixture);
        let signed = scratch.0.join(format!("{fixture}.apk"));
        super::super::run(vec![
            std::env::var("APK_SIGNER").unwrap(),
            "--in".into(),
            template.to_str().unwrap().into(),
            "--out".into(),
            signed.to_str().unwrap().into(),
            "--keystore".into(),
            key.to_str().unwrap().into(),
            "--password-file".into(),
            password.to_str().unwrap().into(),
            "--build-info".into(),
            metadata.to_str().unwrap().into(),
        ])
        .unwrap();
        let java = Path::new(&std::env::var("APK_SIGN_JDK").unwrap()).join("bin/java");
        let jar = Path::new(&std::env::var("APK_SIGN_TOOLS").unwrap()).join("lib/apksigner.jar");
        let verify = Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .args([
                "verify",
                "--verbose",
                "--print-certs",
                "--min-sdk-version",
                "23",
            ])
            .arg(&signed)
            .output()
            .unwrap();
        assert!(
            verify.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&verify.stderr)
        );
        let actual = String::from_utf8(verify.stdout).unwrap();
        assert!(actual.contains("Verified using v1 scheme (JAR signing): true"));
        assert!(actual.contains("Verified using v2 scheme (APK Signature Scheme v2): true"));
        assert!(actual.contains("Verified using v3 scheme (APK Signature Scheme v3): true"));
        let reference = scratch.0.join("reference.apk");
        let sign = Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .args([
                "sign",
                "--min-sdk-version",
                "23",
                "--ks-pass",
                "pass:test123",
                "--ks",
            ])
            .arg(&key)
            .arg("--out")
            .arg(&reference)
            .arg(&template)
            .output()
            .unwrap();
        assert!(
            sign.status.success(),
            "{}",
            String::from_utf8_lossy(&sign.stderr)
        );
        let verify = Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .args(["verify", "--print-certs", "--min-sdk-version", "23"])
            .arg(&reference)
            .output()
            .unwrap();
        assert!(verify.status.success());
        let expected = String::from_utf8(verify.stdout).unwrap();
        let digest = |text: &str| {
            text.lines()
                .find(|line| line.starts_with("Signer #1 certificate SHA-256 digest:"))
                .unwrap()
                .to_owned()
        };
        assert_eq!(digest(&actual), digest(&expected));
        let aligned =
            Command::new(Path::new(&std::env::var("APK_SIGN_TOOLS").unwrap()).join("zipalign"))
                .args(["-c", "-P", "16", "4"])
                .arg(&signed)
                .status()
                .unwrap();
        assert!(aligned.success());
    }
}
