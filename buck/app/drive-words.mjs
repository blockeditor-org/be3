// What web-drive.mjs and web-test.mjs read the same way: drive's leading
// options, a command line split into words as a shell quotes them, and key
// chords in drive's names turned into Playwright's.

const forwarded = (option) =>
    option === "--no-settle" || option.startsWith("--changes=") || option.startsWith("--into=");

// The options before a command: --timeout=SECONDS, and those the app reads,
// which stay in `options` to be passed on with the command.
export function leadingOptions(args) {
    let timeout = 30;
    const options = [];
    let rest = args;
    while (rest[0]?.startsWith("--")) {
        if (rest[0].startsWith("--timeout=")) {
            timeout = Number(rest[0].slice("--timeout=".length));
        } else if (forwarded(rest[0])) {
            options.push(rest[0]);
        } else {
            break;
        }
        rest = rest.slice(1);
    }
    return { timeout, options, rest };
}

export function words(line) {
    const found = [];
    const pattern = /'([^']*)'|"((?:\\.|[^"\\])*)"|(\S+)/g;
    for (const match of line.matchAll(pattern)) {
        found.push(match[1] ?? match[2]?.replace(/\\(.)/g, "$1") ?? match[3]);
    }
    return found;
}

// The lines of a script, `drive -`'s input: each line's words, skipping blank
// lines and # comments.
export function commands(text) {
    return text
        .split("\n")
        .map((line) => ({ line: line.trim(), words: words(line) }))
        .filter(({ words }) => words.length > 0 && !words[0].startsWith("#"));
}

const KEYS = {
    ctrl: "Control",
    control: "Control",
    shift: "Shift",
    alt: "Alt",
    option: "Alt",
    cmd: "Meta",
    meta: "Meta",
    super: "Meta",
    enter: "Enter",
    return: "Enter",
    tab: "Tab",
    escape: "Escape",
    esc: "Escape",
    backspace: "Backspace",
    delete: "Delete",
    space: "Space",
    up: "ArrowUp",
    down: "ArrowDown",
    left: "ArrowLeft",
    right: "ArrowRight",
    home: "Home",
    end: "End",
    pageup: "PageUp",
    pagedown: "PageDown",
};

// A chord such as ctrl+z or shift+Tab, as Playwright's keyboard.press takes it.
export function chord(text) {
    return text
        .split("+")
        .map((key) => KEYS[key.toLowerCase()] ?? (/^f\d+$/i.test(key) ? key.toUpperCase() : key))
        .join("+");
}

// drive's wheel ticks (down, then right) as the pixels Playwright's
// mouse.wheel takes: a tick is a line, beui's SCROLL_LINE.
const SCROLL_LINE = 40;

export function wheelPixels(ticks) {
    const value = Number(ticks ?? 0);
    if (!Number.isInteger(value)) throw new Error(`${ticks} is not a number of ticks`);
    return value * SCROLL_LINE;
}
