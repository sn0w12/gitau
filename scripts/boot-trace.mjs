import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Must match the marker emitted by src/lib/bootstrap/boot-timing.ts and
// crates/tauri/src/boot_trace.rs.
const MARKER = "[boottrace]";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const TARGETS = {
    dev: "gitau-dev.log",
    release: "gitau.log",
};

function usage() {
    console.log(
        [
            "usage: node scripts/boot-trace.mjs [dev|release|<log-file>] [out.json]",
            "",
            "  dev        trace the debug log (default)",
            "  release    trace the release log",
            "  <log-file> trace a specific log file",
        ].join("\n")
    );
}

function logDir() {
    const conf = JSON.parse(
        readFileSync(join(root, "crates/tauri/tauri.conf.json"), "utf8")
    );
    const id = conf.identifier;
    if (process.platform === "win32") {
        const local =
            process.env.LOCALAPPDATA ?? join(homedir(), "AppData", "Local");
        return join(local, id, "logs");
    }
    if (process.platform === "darwin") {
        return join(homedir(), "Library", "Logs", id);
    }
    const data =
        process.env.XDG_DATA_HOME ?? join(homedir(), ".local", "share");
    return join(data, id, "logs");
}

function resolveTarget(arg) {
    const named = Object.hasOwn(TARGETS, arg);
    const path = named ? join(logDir(), TARGETS[arg]) : resolve(arg);
    if (!existsSync(path)) {
        if (named) {
            console.error(`no ${arg} log at ${path}`);
            console.error(
                "run the app first, or pass an explicit log file path"
            );
        } else {
            console.error(`no such log file: ${path}`);
        }
        process.exit(1);
    }
    return path;
}

function parseEvents(text) {
    const events = [];
    for (const line of text.split(/\r?\n/)) {
        const at = line.indexOf(MARKER);
        if (at === -1) continue;
        const body = line.slice(at + MARKER.length).trim();
        try {
            events.push(JSON.parse(body));
        } catch {
            // A line truncated mid-flush at process exit is not an error.
        }
    }
    return events;
}

function metadata(events) {
    const laneNames = new Map();
    for (const event of events) {
        const key = `${event.pid}:${event.tid}`;
        if (!laneNames.has(key)) laneNames.set(key, event.name);
    }
    const rows = [
        { name: "process_name", ph: "M", pid: 1, args: { name: "tauri" } },
        { name: "process_name", ph: "M", pid: 2, args: { name: "webview" } },
    ];
    for (const [key, name] of laneNames) {
        const [pid, tid] = key.split(":").map(Number);
        rows.push({ name: "thread_name", ph: "M", pid, tid, args: { name } });
    }
    return rows;
}

function summarize(events) {
    const start = Math.min(...events.map((event) => event.ts));
    const end = Math.max(...events.map((event) => event.ts + (event.dur ?? 0)));
    const lines = events
        .filter((event) => event.ph === "X" || event.ph === "i")
        .sort((a, b) => a.ts - b.ts)
        .map((event) => {
            const from = ((event.ts - start) / 1000).toFixed(1).padStart(7);
            if (event.ph === "i") {
                return `  ${from}ms        (mark)  ${event.name}`;
            }
            const to = ((event.ts + event.dur - start) / 1000)
                .toFixed(1)
                .padStart(7);
            return `  ${from}ms -> ${to}ms  ${event.name}`;
        });
    return [
        `span ${((end - start) / 1000).toFixed(1)}ms, ${events.length} events`,
        ...lines,
    ].join("\n");
}

function main() {
    const [target, output] = process.argv.slice(2);
    if (target === "-h" || target === "--help") {
        usage();
        return;
    }

    const inputPath = resolveTarget(target ?? "dev");
    const events = parseEvents(readFileSync(inputPath, "utf8")).sort(
        (a, b) => a.ts - b.ts
    );
    if (events.length === 0) {
        console.error(`no ${MARKER} lines in ${inputPath}`);
        process.exit(1);
    }

    // The log is append-only; summarize only the latest run.
    const launches = events.filter((event) => event.name === "rust.launch");
    const runStart = launches.length
        ? launches[launches.length - 1].ts
        : Number.NEGATIVE_INFINITY;
    const run =
        launches.length > 1
            ? events.filter((event) => event.ts >= runStart)
            : events;

    const label = basename(inputPath).replace(/\.[^.]+$/, "");
    const outPath = resolve(
        output ??
            join(
                root,
                "node_modules/.cache/gitau-boot-trace",
                `${label}.trace.json`
            )
    );
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(
        outPath,
        JSON.stringify({ traceEvents: [...metadata(run), ...run] })
    );

    if (launches.length > 1) {
        console.log(`latest of ${launches.length} runs in ${inputPath}`);
    }
    console.log(summarize(run));
    console.log(
        `\nwrote ${outPath} (open in chrome://tracing or ui.perfetto.dev)`
    );
}

main();
