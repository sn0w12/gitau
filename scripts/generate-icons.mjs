import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauriBin = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");

const CHANNELS = [
    { source: "images/icons/default.png", output: "crates/tauri/icons" },
    { source: "images/icons/dev.png", output: "crates/tauri/icons/dev" },
    {
        source: "images/icons/preview.png",
        output: "crates/tauri/icons/preview",
    },
];

const DESKTOP_FILES = new Set([
    "32x32.png",
    "64x64.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.icns",
    "icon.ico",
    "icon.png",
]);

function frameWidth(entryOffset, buffer) {
    return buffer[entryOffset] === 0 ? 256 : buffer[entryOffset];
}

// tauri-codegen embeds ICO entry 0 as the window icon and Windows scales that
// single bitmap to every taskbar size, so the largest frame must sort first.
function reorderIco(file) {
    const buffer = readFileSync(file);
    const count = buffer.readUInt16LE(4);
    const entries = [];
    for (let index = 0; index < count; index += 1) {
        const entryOffset = 6 + index * 16;
        const dataLength = buffer.readUInt32LE(entryOffset + 8);
        const dataOffset = buffer.readUInt32LE(entryOffset + 12);
        entries.push({
            width: frameWidth(entryOffset, buffer),
            entry: buffer.subarray(entryOffset, entryOffset + 16),
            data: buffer.subarray(dataOffset, dataOffset + dataLength),
        });
    }
    entries.sort((a, b) => b.width - a.width);

    const header = Buffer.alloc(6 + count * 16);
    header.writeUInt16LE(0, 0);
    header.writeUInt16LE(1, 2);
    header.writeUInt16LE(count, 4);

    const chunks = [header];
    let dataOffset = header.length;
    for (const [index, { entry, data }] of entries.entries()) {
        const rewritten = Buffer.from(entry);
        rewritten.writeUInt32LE(data.length, 8);
        rewritten.writeUInt32LE(dataOffset, 12);
        rewritten.copy(header, 6 + index * 16);
        chunks.push(data);
        dataOffset += data.length;
    }
    writeFileSync(file, Buffer.concat(chunks));
}

function pruneToDesktopFiles(output) {
    for (const entry of readdirSync(output, { withFileTypes: true })) {
        if (entry.isDirectory() || !DESKTOP_FILES.has(entry.name)) {
            rmSync(join(output, entry.name), { recursive: true, force: true });
        }
    }
}

for (const { source, output } of CHANNELS) {
    execFileSync(process.execPath, [tauriBin, "icon", source, "-o", output], {
        cwd: root,
        stdio: "inherit",
    });
    reorderIco(join(root, output, "icon.ico"));
    if (output !== "crates/tauri/icons") {
        pruneToDesktopFiles(join(root, output));
    }
    console.log(`Generated ${output} from ${source}`);
}
