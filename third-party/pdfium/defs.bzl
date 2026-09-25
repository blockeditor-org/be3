load("@root//buck/platforms:cross.bzl", "per_cross_platform")

# platform: (asset, library inside it, sha256, size). The host is linux_x86_64.
PDFIUM = {
    "linux_arm64": ("pdfium-linux-arm64", "lib/libpdfium.so", "e98400ef5f005f27cfba5c14f72d464e25187298f04950de46646033cf24cef0", 3660772),
    "linux_x86_64": ("pdfium-linux-x64", "lib/libpdfium.so", "eb142f416aed3a72fc5a02dbd5884868a16cb99dc0cf53e6bdd64afbf67b05f4", 3739655),
    "macos_arm64": ("pdfium-mac-arm64", "lib/libpdfium.dylib", "61424884d4a7f153b808deba6437848e4400834ce30aaf95d3050da44df8f420", 3479085),
    "macos_x86_64": ("pdfium-mac-x64", "lib/libpdfium.dylib", "a93d44238e05de20028446561b951d50988b849efbbe56fe40c0d376c05b45e8", 3673663),
    "windows_arm64": ("pdfium-win-arm64", "bin/pdfium.dll", "6c9ac0ddc69edd8a18d47b95098a5b843eaed5c5bbdcb9587a18c196457449f8", 3592415),
    "windows_x86_64": ("pdfium-win-x64", "bin/pdfium.dll", "78a17d9a5f14467631c26a3ac8741b27a0471ecc05bd6a119b523598160a0537", 3818370),
}

def _library(platform):
    asset, library, _, _ = PDFIUM[platform]
    return ["//third-party/pdfium:{}[{}]".format(asset, library)]

# The PDFium library for the platform being built, as a list for the app to
# stage: empty for Android, whose APK carries none.
def pdfium_library():
    return per_cross_platform(
        _library("linux_x86_64"),
        lambda platform: _library(platform) if platform in PDFIUM else [],
    )
