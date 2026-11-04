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

// A line split into words as be-drive's `words` splits it: single quotes keep
// everything to the next one, double quotes take backslash escapes, a
// backslash outside quotes escapes the next character, and an unclosed quote
// is an error.
export function words(line) {
    const found = [];
    let word = "";
    let started = false;
    const characters = [...line];
    for (let at = 0; at < characters.length; at++) {
        const character = characters[at];
        if (character === "'" || character === '"') {
            started = true;
            for (at++; ; at++) {
                if (at >= characters.length) throw new Error(`an unclosed quote in: ${line}`);
                if (characters[at] === character) break;
                if (character === '"' && characters[at] === "\\") at++;
                word += characters[at] ?? "";
            }
        } else if (character === "\\") {
            started = true;
            at++;
            word += characters[at] ?? "";
        } else if (/\s/.test(character)) {
            if (started) found.push(word);
            word = "";
            started = false;
        } else {
            started = true;
            word += character;
        }
    }
    if (started) found.push(word);
    return found;
}

// The lines of a script, `drive -`'s input, as be-drive's `commands` reads
// them: each line's words, skipping blank lines and lines starting with #,
// before their quotes are read.
export function commands(text) {
    return text
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.length > 0 && !line.startsWith("#"))
        .map((line) => ({ line, words: words(line) }))
        .filter(({ words }) => words.length > 0);
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
    plus: "Equal",
    equal: "Equal",
    minus: "Minus",
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

export class Failure extends Error {}

// Where each target shows, as native drive aims input: `ask(command, target)`
// sends `reveal TARGET` or `shown TARGET` (the HTML core's automate and the
// drive layer both answer them). A target a scroll cuts is revealed, at most
// once, and every target is found again after it, since that scroll may have
// moved the ones before. Each place is in CSS pixels, the tree's divided by ratio.
export async function shownPlaces(ask, targets, ratio) {
    const revealed = new Set();
    for (;;) {
        const found = [];
        for (const target of targets) {
            const reply = await ask(revealed.has(target) ? "shown" : "reveal", target);
            if (reply.trim().endsWith(" revealed")) {
                revealed.add(target);
                break;
            }
            const place = reply.match(/at (-?[\d.]+),(-?[\d.]+) size ([\d.]+)x([\d.]+)/);
            if (!place) throw new Failure(`${target} has no place on screen: ${reply.trim()}`);
            const [x, y, width, height] = place.slice(1).map((value) => Number(value) / ratio);
            found.push({ x, y, width, height });
        }
        if (found.length === targets.length) return found;
    }
}
