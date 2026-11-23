import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// sharun_cef downloads quick-sharun from a stale fork whose forced deploy
// list is evaluated without eval, so the modules Chromium dlopens have to
// reach it as ELF inputs. The target must start with /usr/lib for sharun_cef
// to forward the source, and the source must live under /usr/lib for
// quick-sharun to strip the prefix and deploy it flat into lib/.
const NSS_LIBS = [
    "libnss3.so",
    "libnssutil3.so",
    "libsoftokn3.so",
    "libfreebl3.so",
    "libfreeblpriv3.so",
    "libnssckbi.so",
];

const multiarch = execFileSync("gcc", ["-print-multiarch"], {
    encoding: "utf8",
}).trim();

const files = {};
for (const lib of NSS_LIBS) {
    const source = join("/usr/lib", multiarch, lib);
    if (!existsSync(source)) {
        throw new Error(`Missing ${source}`);
    }
    files[`/usr/lib/gitau-nss/${lib}`] = source;
}

mkdirSync(".appimage", { recursive: true });
writeFileSync(
    ".appimage/tauri.appimage.conf.json",
    JSON.stringify({ bundle: { linux: { appimage: { files } } } }, null, 2)
);
