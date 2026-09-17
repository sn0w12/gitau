import { fileURLToPath } from "node:url";

import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import viteReact, { reactCompilerPreset } from "@vitejs/plugin-react";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

// react-scan must evaluate before React to hook the DevTools global, and
// prepending it here instead of in main.tsx keeps the import sorter from
// moving it below React. apply: "serve" keeps it out of production builds.
function reactScanDev(): Plugin {
    return {
        name: "gitau:react-scan-dev",
        apply: "serve",
        transform(code, id) {
            if (process.env.VITEST) return;
            const path = id.split("?")[0].replace(/\\/g, "/");
            if (!path.endsWith("/src/main.tsx")) return;
            return `import { scan } from "react-scan";\nscan({ enabled: true });\n${code}`;
        },
    };
}

const config = defineConfig({
    resolve: {
        alias: {
            "@": fileURLToPath(new URL("./src", import.meta.url)),
        },
    },
    plugins: [
        tailwindcss(),
        babel({ presets: [reactCompilerPreset()] }),
        viteReact(),
        reactScanDev(),
    ],
    server: {
        port: 3000,
        strictPort: true,
        watch: {
            ignored: ["**/target/**"],
        },
    },
    test: {
        include: ["tests/**/*.test.{ts,tsx}"],
        setupFiles: ["./tests/setup.ts"],
        alias: {
            // The real package imports a stylesheet and defines custom
            // elements at module scope, which Vitest cannot load. Tests never
            // assert the animation, so swap in a plain-text stub.
            "@scritto/react": fileURLToPath(
                new URL("./tests/stubs/scritto-react.tsx", import.meta.url)
            ),
        },
    },
});

export default config;
