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
    let template = directory.join("template.apk");
    let metadata = directory.join("build-info.json");
    let password = directory.join("password");
    fs::write(&metadata, b"{\"id\":\"example\"}").unwrap();
    fs::write(&password, b"test123").unwrap();
    let manifest = directory.join("AndroidManifest.xml");
    let linked = directory.join("manifest.apk");
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
    let java = Path::new(&std::env::var("APK_SIGN_JDK").unwrap()).join("bin/java");
    let keytool = Path::new(&std::env::var("APK_SIGN_JDK").unwrap()).join("bin/keytool");
    for (fixture, store_type, option) in [
        ("test.jks", "JKS", "-J-Dkeystore.pkcs12.legacy=false"),
        ("test.p12", "PKCS12", "-J-Dkeystore.pkcs12.legacy=false"),
        ("test-legacy.p12", "PKCS12", "-J-Dkeystore.pkcs12.legacy"),
    ] {
        let made = Command::new(&keytool)
            .arg(option)
            .args(["-genkeypair", "-keystore"])
            .arg(directory.join(fixture))
            .args([
                "-storetype",
                store_type,
                "-storepass",
                "test123",
                "-keypass",
                "test123",
                "-alias",
                "test",
                "-dname",
                "CN=Test",
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "1000",
            ])
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
    }
    let generated = directory.join("generated.p12");
    let encoded = directory.join("generated.base64");
    super::super::generate_key(&generated, 2048).unwrap();
    let original_key = fs::read(&generated).unwrap();
    assert!(super::super::generate_key(&generated, 2048).is_err());
    assert_eq!(fs::read(&generated).unwrap(), original_key);
    super::super::save_key(&encoded, &original_key).unwrap();
    for fixture in [
        "test.jks",
        "test.p12",
        "test-legacy.p12",
        "generated.p12",
        "generated.base64",
    ] {
        let is_generated = fixture.starts_with("generated");
        let key = directory.join(fixture);
        let pass = if is_generated { "android" } else { "test123" };
        fs::write(&password, pass).unwrap();
        let signed = directory.join(format!("{fixture}.apk"));
        super::super::run(vec![
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
        let jar = Path::new(&std::env::var("APK_SIGN_TOOLS").unwrap()).join("lib/apksigner.jar");
        let verify = Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .args([
                "verify",
                "--verbose",
                "--print-certs",
                "--min-sdk-version",
                "24",
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
        assert!(actual.contains("Verified using v1 scheme (JAR signing): false"));
        assert!(actual.contains("Verified using v2 scheme (APK Signature Scheme v2): true"));
        assert!(actual.contains("Verified using v3 scheme (APK Signature Scheme v3): false"));
        assert!(!actual.contains("WARNING"), "{actual}");
        assert!(!String::from_utf8_lossy(&verify.stderr).contains("WARNING"));
        let mut archive = zip::ZipArchive::new(fs::File::open(&signed).unwrap()).unwrap();
        assert!(archive.by_name("META-INF/MANIFEST.MF").is_err());
        let reference = directory.join("reference.apk");
        let reference_key = directory.join("reference-key.p12");
        fs::write(&reference_key, super::super::read_key(&key).unwrap()).unwrap();
        let sign = Command::new(&java)
            .arg("-jar")
            .arg(&jar)
            .args([
                "sign",
                "--min-sdk-version",
                "24",
                "--ks-pass",
                &format!("pass:{pass}"),
                "--ks",
            ])
            .arg(&reference_key)
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
            .args(["verify", "--print-certs", "--min-sdk-version", "24"])
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
    fs::remove_dir_all(directory).unwrap();
}
