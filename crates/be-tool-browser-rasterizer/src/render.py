import json
import os
import sys
from urllib.parse import urljoin, urldefrag

try:
    from playwright.sync_api import sync_playwright
except ImportError:
    sys.exit("Install the preview runtime: python3 -m pip install playwright==1.64.0 && python3 -m playwright install --with-deps chromium")

snapshot = json.loads(sys.stdin.buffer.read())
origin = "https://snapshot.invalid/"
files = {
    urljoin(origin, url): (
        file["content_type"],
        "".join(file["text"]).encode() if file["text"] is not None else bytes(file["bytes"]),
    )
    for url, file in snapshot["files"].items()
}
files[origin] = ("text/html; charset=utf-8", "".join(snapshot["document"]).encode())
missing = set()
errors = []

with sync_playwright() as playwright:
    browser = playwright.chromium.launch(executable_path=os.environ.get("BE3_CHROME") or None)
    context = browser.new_context(
        viewport={"width": snapshot["size"][0], "height": snapshot["size"][1]},
        device_scale_factor=1,
        locale="en-US",
        timezone_id="UTC",
        color_scheme="light",
        reduced_motion="reduce",
        service_workers="block",
        offline=True,
    )

    def route_request(route):
        url = urldefrag(route.request.url)[0]
        resource = files.get(url)
        if resource is None:
            missing.add(url)
            route.abort()
        else:
            content_type, body = resource
            route.fulfill(content_type=content_type, body=body, headers={"access-control-allow-origin": "*"})

    context.route("**/*", route_request)
    page = context.new_page()
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.goto(origin, wait_until="load", timeout=30000)
    page.evaluate("document.fonts.ready")
    broken_images = page.evaluate("""() => Promise.all(Array.from(document.images, async image => {
        try { await image.decode(); return null; }
        catch { return image.currentSrc || image.src; }
    }))""")
    image = page.screenshot(animations="disabled", caret="hide", timeout=30000)
    browser.close()

if missing:
    sys.exit("Snapshot is missing embedded resources:\n" + "\n".join(sorted(missing)))
if any(broken_images):
    sys.exit("Snapshot images failed to decode:\n" + "\n".join(url for url in broken_images if url))
if errors:
    sys.exit("Snapshot scripts failed:\n" + "\n".join(errors))
sys.stdout.buffer.write(image)
