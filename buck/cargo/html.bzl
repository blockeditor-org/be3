load("@root//buck/platforms:cross.bzl", "HTML_PLANS")
load(":crates.bzl", "crates")

# Every first-party library cargo plans for beui's HTML core, beside the
# platform it is built for there: what //:check and ./scripts/verify check on
# that core, so that the crates which build on it keep building.
def html_libraries() -> list[(str, str)]:
    found = []
    for package, crate in sorted(crates.items()):
        if crate["library"] == None or crate["library"]["proc_macro"]:
            continue
        for plan, platform in sorted(HTML_PLANS.items()):
            if plan in crate["platforms"]:
                found.append(("root//{}:{}".format(package, crate["name"]), platform))
    return found
