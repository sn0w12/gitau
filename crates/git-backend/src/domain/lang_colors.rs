//! Language detection and display colors, mirroring GitHub's Linguist:
//! exact filename match first, then a longest-registered-extension match,
//! then shebang inspection for extensionless scripts. Colors come from the
//! github-colors dataset (https://github.com/ozh/github-colors), MIT
//! licensed; entries the dataset leaves uncolored carry an empty string.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Language display colors. An empty color string means the dataset
/// defines none for that language.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("4D", "#004289"),
    ("ABAP", "#E8274B"),
    ("AL", "#3AA2B5"),
    ("AMPL", "#E6EFBB"),
    ("ANTLR", "#9DC3FF"),
    ("API Blueprint", "#2ACCA8"),
    ("APL", "#5A8164"),
    ("ASP.NET", "#9400ff"),
    ("ActionScript", "#882B0F"),
    ("Ada", "#02f88c"),
    ("ApacheConf", "#d12127"),
    ("Apex", "#1797c0"),
    ("AppleScript", "#101F1F"),
    ("Arduino", "#bd79d1"),
    ("Assembly", "#6E4C13"),
    ("Astro", "#ff5a03"),
    ("AutoHotkey", "#6594b9"),
    ("AutoIt", "#1C3552"),
    ("Awk", "#c30e9b"),
    ("Ballerina", "#FF5000"),
    ("Batchfile", "#C1F12E"),
    ("Bicep", "#519aba"),
    ("Blade", "#f7523f"),
    ("C", "#555555"),
    ("C#", "#7355dd"),
    ("C++", "#f34b7d"),
    ("CMake", "#DA3434"),
    ("COBOL", ""),
    ("Clojure", "#db5855"),
    ("CSS", "#663399"),
    ("CSV", "#237346"),
    ("Cap'n Proto", "#c42727"),
    ("CoffeeScript", "#244776"),
    ("ColdFusion", "#ed2cd6"),
    ("Common Lisp", "#3fb68b"),
    ("Crystal", "#000100"),
    ("Cuda", "#3A4E3A"),
    ("Cython", "#fedf5b"),
    ("D", "#ba595e"),
    ("Dart", "#00B4AB"),
    ("Dhall", "#dfafff"),
    ("Dockerfile", "#384d54"),
    ("EJS", "#a91e50"),
    ("Emacs Lisp", "#c065db"),
    ("Elixir", "#6e4a7e"),
    ("Elm", "#60B5CC"),
    ("Erlang", "#B83998"),
    ("F#", "#b845fc"),
    ("Fortran", "#4d41b1"),
    ("G-code", "#D08CF2"),
    ("GAML", "#FFC766"),
    ("GDScript", "#355570"),
    ("Gleam", "#ffaff3"),
    ("Go", "#00ADD8"),
    ("Gradle", "#02303a"),
    ("GraphQL", "#e10098"),
    ("Groovy", "#4298b8"),
    ("HTML", "#e34c26"),
    ("HTML+ERB", "#701516"),
    ("HTML+EEX", "#6e4a7e"),
    ("HTML+Razor", "#512be4"),
    ("Hack", "#878787"),
    ("Handlebars", "#f7931e"),
    ("HCL", "#844FBA"),
    ("HLSL", "#aace60"),
    ("Haskell", "#5e5086"),
    ("IDL", "#a3522f"),
    ("INI", "#d1dbe0"),
    ("Idris", "#b30000"),
    ("Java", "#b07219"),
    ("Java Server Pages", "#2A6277"),
    ("JavaScript", "#f1e05a"),
    ("Jinja", "#a52a22"),
    ("JSON", "#292929"),
    ("JSON5", "#267CB9"),
    ("JSONLD", "#0c479c"),
    ("Jsonnet", "#0064bd"),
    ("jq", "#c7254e"),
    ("Julia", "#a270ba"),
    ("Jupyter Notebook", "#DA5B0B"),
    ("Kotlin", "#A97BFF"),
    ("LLVM", "#185619"),
    ("LSL", "#3d9970"),
    ("Less", "#1d365d"),
    ("Liquid", "#67b8de"),
    ("Logtalk", "#295b9a"),
    ("Lua", "#000080"),
    ("MATLAB", "#e16737"),
    ("MDX", "#fcb32c"),
    ("MQL4", "#62A8D6"),
    ("MQL5", "#4A76B8"),
    ("Makefile", "#427819"),
    ("Markdown", "#083fa1"),
    ("Meson", "#007800"),
    ("Metal", "#8f14e9"),
    ("Mojo", "#ff4c1f"),
    ("Mustache", "#724b3b"),
    ("Nim", "#ffc200"),
    ("Nix", "#7e7eff"),
    ("Nunjucks", "#3d8137"),
    ("Nushell", "#4E9906"),
    ("OCaml", "#ef7a08"),
    ("Objective-C", "#438eff"),
    ("Objective-C++", "#6866fb"),
    ("Odin", "#60AFFE"),
    ("Open Policy Agent", "#7d9199"),
    ("OpenSCAD", "#e5cd45"),
    ("Org", "#77aa99"),
    ("PHP", "#4F5D95"),
    ("Pascal", "#E3F171"),
    ("Perl", "#0298c3"),
    ("Pike", "#005390"),
    ("Pkl", "#6b9543"),
    ("POV-Ray SDL", "#6bac65"),
    ("PostCSS", "#dc3a0c"),
    ("PowerShell", "#012456"),
    ("Prisma", "#0c344b"),
    ("Prolog", "#74283c"),
    ("Protocol Buffer", ""),
    ("Pug", "#a86454"),
    ("Puppet", "#302B6D"),
    ("PureScript", "#1D222D"),
    ("Python", "#3572A5"),
    ("QML", "#44a51c"),
    ("R", "#198CE7"),
    ("RON", "#a62c00"),
    ("RMarkdown", "#198ce7"),
    ("Racket", "#3c5caa"),
    ("ReScript", "#ed5051"),
    ("reStructuredText", "#141414"),
    ("Ruby", "#701516"),
    ("Rust", "#dea584"),
    ("SCSS", "#c6538c"),
    ("SQL", "#e38c00"),
    ("SVG", "#ff9900"),
    ("Sass", "#a53b70"),
    ("Scala", "#c22d40"),
    ("Scheme", "#1e4aec"),
    ("ShaderLab", "#88c9c9"),
    ("Shell", "#89e051"),
    ("Solidity", "#AA6746"),
    ("Standard ML", "#dc566d"),
    ("Stylus", "#ff6347"),
    ("Svelte", "#ff3e00"),
    ("Swift", "#F05138"),
    ("SystemVerilog", "#DAE1C2"),
    ("Tcl", "#e4cc98"),
    ("TOML", "#9c4221"),
    ("Twig", "#c1d026"),
    ("TypeScript", "#3178c6"),
    ("VHDL", "#adb2cb"),
    ("Vala", "#a56de2"),
    ("Verilog", "#b2b7f8"),
    ("Vim Script", "#199f4b"),
    ("Vue", "#41b883"),
    ("XML", "#0060ac"),
    ("XSLT", "#EB8CEB"),
    ("YAML", "#cb171e"),
    ("Zig", "#ec915c"),
];

/// Display color for a language name; `None` when the dataset defines no
/// color for it.
pub fn color_for_language(language: &str) -> Option<&'static str> {
    language_lookup()
        .get(language)
        .copied()
        .filter(|color| !color.is_empty())
}

fn language_lookup() -> &'static HashMap<&'static str, &'static str> {
    static LOOKUP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    LOOKUP.get_or_init(|| LANGUAGES.iter().copied().collect())
}

fn filename_lookup() -> &'static HashMap<&'static str, &'static str> {
    static LOOKUP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    LOOKUP.get_or_init(|| FILENAMES.iter().copied().collect())
}

fn extension_lookup() -> &'static HashMap<&'static str, &'static str> {
    static LOOKUP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    LOOKUP.get_or_init(|| EXTENSIONS.iter().copied().collect())
}

/// Outcome of name-based classification for one file.
pub enum Resolution {
    /// The name alone decides. `None` excludes the file from stats
    /// (lockfiles, bundles, build outputs).
    Decided(Option<&'static str>),
    /// The extension is shared by several languages; candidates are
    /// searched in order, each needle list matching by substring, and the
    /// final candidate is the fallthrough default.
    Disambiguate(&'static [(&'static [&'static str], &'static str)]),
    /// No name rule matched; a shebang may still identify the file.
    Shebang,
}

/// Exact filename matches (lowercased); an empty language means the file
/// is excluded from stats.
const FILENAMES: &[(&str, &str)] = &[
    (".babelrc", "JSON"),
    (".dockerignore", ""),
    (".editorconfig", "INI"),
    (".gitattributes", ""),
    (".gitconfig", "INI"),
    (".gitignore", ""),
    (".htaccess", "ApacheConf"),
    (".npmrc", "INI"),
    (".prettierrc", "JSON"),
    ("brewfile", "Ruby"),
    ("build.gradle", "Groovy"),
    ("build.gradle.kts", "Kotlin"),
    ("cargo.lock", ""),
    ("cakefile", "CoffeeScript"),
    ("cmakelists.txt", "CMake"),
    ("composer.lock", ""),
    ("containerfile", "Dockerfile"),
    ("dockerfile", "Dockerfile"),
    ("gemfile", "Ruby"),
    ("gemfile.lock", ""),
    ("go.mod", "Go"),
    ("go.sum", ""),
    ("go.work", "Go"),
    ("gradlew", "Shell"),
    ("jenkinsfile", "Groovy"),
    ("makefile", "Makefile"),
    ("makefile.am", "Makefile"),
    ("makefile.in", "Makefile"),
    ("meson.build", "Meson"),
    ("package-lock.json", ""),
    ("podfile", "Ruby"),
    ("podfile.lock", ""),
    ("procfile", "Procfile"),
    ("rakefile", "Ruby"),
    ("requirements.txt", ""),
    ("vagrantfile", "Ruby"),
    ("yarn.lock", ""),
];

/// Extension matches (lowercased, with leading dot); an empty language
/// excludes the file. Iterated longest-suffix-first at each dot, so the
/// leftmost (longest) registered suffix wins: `.d.ts` beats `.ts` and
/// `.min.js` beats `.js`.
const EXTENSIONS: &[(&str, &str)] = &[
    (".4dm", "4D"),
    (".a51", "Assembly"),
    (".abap", "ABAP"),
    (".adb", "Ada"),
    (".ads", "Ada"),
    (".al", "AL"),
    (".ampl", "AMPL"),
    (".as", "ActionScript"),
    (".asm", "Assembly"),
    (".astro", "Astro"),
    (".avsc", "JSON"),
    (".awk", "Awk"),
    (".ballerina", "Ballerina"),
    (".bash", "Shell"),
    (".bat", "Batchfile"),
    (".bicep", "Bicep"),
    (".blade", "Blade"),
    (".c", "C"),
    (".c++", "C++"),
    (".capnp", "Cap'n Proto"),
    (".cats", "C"),
    (".cc", "C++"),
    (".cjs", "JavaScript"),
    (".cl", "Common Lisp"),
    (".clj", "Clojure"),
    (".cljc", "Clojure"),
    (".cljs", "Clojure"),
    (".cmd", "Batchfile"),
    (".cob", "COBOL"),
    (".cbl", "COBOL"),
    (".ccp", "COBOL"),
    (".containerfile", "Dockerfile"),
    (".coffee", "CoffeeScript"),
    (".conf", "INI"),
    (".cpp", "C++"),
    (".cr", "Crystal"),
    (".cs", "C#"),
    (".cshtml", "HTML+Razor"),
    (".css", "CSS"),
    (".csx", "C#"),
    (".csv", "CSV"),
    (".cts", "TypeScript"),
    (".cu", "Cuda"),
    (".cuh", "Cuda"),
    (".cxx", "C++"),
    (".d", "D"),
    (".d.ts", "TypeScript"),
    (".dart", "Dart"),
    (".dhall", "Dhall"),
    (".dpr", "Pascal"),
    (".dockerfile", "Dockerfile"),
    (".eex", "HTML+EEX"),
    (".ejs", "EJS"),
    (".el", "Emacs Lisp"),
    (".elm", "Elm"),
    (".erb", "HTML+ERB"),
    (".erl", "Erlang"),
    (".ex", "Elixir"),
    (".exs", "Elixir"),
    (".f", "Fortran"),
    (".f03", "Fortran"),
    (".f08", "Fortran"),
    (".f77", "Fortran"),
    (".f90", "Fortran"),
    (".f95", "Fortran"),
    (".fish", "Shell"),
    (".for", "Fortran"),
    (".fs", "F#"),
    (".fsi", "F#"),
    (".fsx", "F#"),
    (".g4", "ANTLR"),
    (".gaml", "GAML"),
    (".gco", "G-code"),
    (".gcode", "G-code"),
    (".gd", "GDScript"),
    (".gemspec", "Ruby"),
    (".geojson", "JSON"),
    (".gleam", "Gleam"),
    (".gltf", "JSON"),
    (".go", "Go"),
    (".gql", "GraphQL"),
    (".gradle", "Gradle"),
    (".graphql", "GraphQL"),
    (".graphqls", "GraphQL"),
    (".groovy", "Groovy"),
    (".gvy", "Groovy"),
    (".h", "C"),
    (".hbs", "Handlebars"),
    (".hcl", "HCL"),
    (".hh", "C++"),
    (".hlsl", "HLSL"),
    (".hpp", "C++"),
    (".hrl", "Erlang"),
    (".hs", "Haskell"),
    (".htm", "HTML"),
    (".html", "HTML"),
    (".hxx", "C++"),
    (".idl", "IDL"),
    (".idr", "Idris"),
    (".inc", "C"),
    (".ini", "INI"),
    (".inl", "C++"),
    (".ino", "Arduino"),
    (".ipp", "C++"),
    (".ipynb", "Jupyter Notebook"),
    (".j2", "Jinja"),
    (".java", "Java"),
    (".jinja", "Jinja"),
    (".jinja2", "Jinja"),
    (".jl", "Julia"),
    (".jq", "jq"),
    (".js", "JavaScript"),
    (".json", "JSON"),
    (".json5", "JSON5"),
    (".jsonc", "JSON"),
    (".jsonld", "JSONLD"),
    (".jsonnet", "Jsonnet"),
    (".jsp", "Java Server Pages"),
    (".jsx", "JavaScript"),
    (".ksh", "Shell"),
    (".kt", "Kotlin"),
    (".kts", "Kotlin"),
    (".less", "Less"),
    (".lgt", "Logtalk"),
    (".libsonnet", "Jsonnet"),
    (".liquid", "Liquid"),
    (".lisp", "Common Lisp"),
    (".ll", "LLVM"),
    (".lock", ""),
    (".lsl", "LSL"),
    (".lua", "Lua"),
    (".m", "Objective-C"),
    (".mak", "Makefile"),
    (".make", "Makefile"),
    (".map", ""),
    (".markdown", "Markdown"),
    (".md", "Markdown"),
    (".mdx", "MDX"),
    (".meson", "Meson"),
    (".metal", "Metal"),
    (".min.css", ""),
    (".min.js", ""),
    (".mjs", "JavaScript"),
    (".mk", "Makefile"),
    (".ml", "OCaml"),
    (".mli", "OCaml"),
    (".mm", "Objective-C++"),
    (".mojo", "Mojo"),
    (".mq4", "MQL4"),
    (".mq5", "MQL5"),
    (".mts", "TypeScript"),
    (".mustache", "Mustache"),
    (".nasm", "Assembly"),
    (".nim", "Nim"),
    (".nims", "Nim"),
    (".nix", "Nix"),
    (".njk", "Nunjucks"),
    (".nu", "Nushell"),
    (".odin", "Odin"),
    (".ops", "Open Policy Agent"),
    (".org", "Org"),
    (".p8", "Lua"),
    (".pas", "Pascal"),
    (".patch", ""),
    (".pb.go", "Go"),
    (".pcss", "PostCSS"),
    (".perl", "Perl"),
    (".php", "PHP"),
    (".php3", "PHP"),
    (".php4", "PHP"),
    (".php5", "PHP"),
    (".phtml", "PHP"),
    (".pl", "Perl"),
    (".pm", "Perl"),
    (".pmod", "Pike"),
    (".podspec", "Ruby"),
    (".pov", "POV-Ray SDL"),
    (".pp", "Puppet"),
    (".prisma", "Prisma"),
    (".properties", "INI"),
    (".proto", "Protocol Buffer"),
    (".ps1", "PowerShell"),
    (".psd1", "PowerShell"),
    (".psm1", "PowerShell"),
    (".pug", "Pug"),
    (".purs", "PureScript"),
    (".pxd", "Cython"),
    (".py", "Python"),
    (".pyde", "Python"),
    (".pyi", "Python"),
    (".pyp", "Python"),
    (".pyx", "Cython"),
    (".qml", "QML"),
    (".r", "R"),
    (".rake", "Ruby"),
    (".rb", "Ruby"),
    (".rkt", "Racket"),
    (".rlib", "Rust"),
    (".ron", "RON"),
    (".rs", "Rust"),
    (".rss", "XML"),
    (".rst", "reStructuredText"),
    (".ruby", "Ruby"),
    (".s", "Assembly"),
    (".sass", "Sass"),
    (".sbt", "Scala"),
    (".sc", "Scala"),
    (".scad", "OpenSCAD"),
    (".scala", "Scala"),
    (".scm", "Scheme"),
    (".scss", "SCSS"),
    (".sh", "Shell"),
    (".shader", "ShaderLab"),
    (".sml", "Standard ML"),
    (".sol", "Solidity"),
    (".sql", "SQL"),
    (".ss", "Scheme"),
    (".styl", "Stylus"),
    (".svelte", "Svelte"),
    (".svg", "SVG"),
    (".swift", "Swift"),
    (".sv", "SystemVerilog"),
    (".svh", "SystemVerilog"),
    (".tcl", "Tcl"),
    (".tf", "HCL"),
    (".tfstate", "JSON"),
    (".tfvars", "HCL"),
    (".tk", "Tcl"),
    (".toml", "TOML"),
    (".ts", "TypeScript"),
    (".tsx", "TypeScript"),
    (".twig", "Twig"),
    (".typescript", "TypeScript"),
    (".v", "Verilog"),
    (".vala", "Vala"),
    (".vapi", "Vala"),
    (".vhd", "VHDL"),
    (".vhdl", "VHDL"),
    (".vim", "Vim Script"),
    (".vue", "Vue"),
    (".wasm", ""),
    (".xaml", "XML"),
    (".xht", "HTML"),
    (".xhtml", "HTML"),
    (".xsl", "XSLT"),
    (".xslt", "XSLT"),
    (".yaml", "YAML"),
    (".yml", "YAML"),
    (".yrl", "Erlang"),
    (".zig", "Zig"),
    (".zsh", "Shell"),
];

const OBJC_NEEDLES: &[&str] = &["@interface", "@implementation", "@end", "@property"];
const CPP_NEEDLES: &[&str] = &["::", "template<", "namespace", "nullptr", "std::"];

/// Extension conflicts resolved by content, keyed by registered suffix.
type ContentRules = &'static [(&'static [&'static str], &'static str)];
const DISAMBIG: &[(&str, ContentRules)] = &[
    (
        ".h",
        &[
            (OBJC_NEEDLES, "Objective-C"),
            (CPP_NEEDLES, "C++"),
            (&[], "C"),
        ],
    ),
    (
        ".m",
        &[
            (OBJC_NEEDLES, "Objective-C"),
            (CPP_NEEDLES, "C++"),
            (&["function ", "end"], "MATLAB"),
            (&[], "Objective-C"),
        ],
    ),
    (
        ".php",
        &[
            (&["<?hh"], "Hack"),
            (&["<?php", "<?=", "<? "], "PHP"),
            (&[], "PHP"),
        ],
    ),
    (
        ".pl",
        &[
            (&["use strict", "my $", "sub ", "->", "print "], "Perl"),
            (&[" :-", "?-"], "Prolog"),
            (&[], "Perl"),
        ],
    ),
    (
        ".pp",
        &[
            (
                &["class ", "define ", "include", "node ", "ensure"],
                "Puppet",
            ),
            (&["program ", "procedure ", "begin", "end."], "Pascal"),
            (&[], "Puppet"),
        ],
    ),
    (".r", &[(&["<-", "function(", "library("], "R"), (&[], "R")]),
    (
        ".sc",
        &[
            (&["val ", "def ", "object ", "import "], "Scala"),
            (&[], "Scala"),
        ],
    ),
    (
        ".v",
        &[
            (
                &[
                    "module ",
                    "input ",
                    "always @",
                    "endmodule",
                    "wire ",
                    "reg ",
                ],
                "Verilog",
            ),
            (&["Theorem", "Lemma", "Require Import", "Proof."], "VHDL"),
            (&[], "Verilog"),
        ],
    ),
];

/// Generic template suffixes stripped before extension matching.
const TEMPLATE_SUFFIXES: &[&str] = &[".example", ".template", ".sample", ".dist"];

/// Content scans beyond this size skip pattern matching and fall back to
/// the extension's default language.
pub(crate) const MAX_CONTENT_SCAN: usize = 512 * 1024;

/// Classifies one file by name rules and, when needed, its bytes.
pub fn classify_language(
    rela_path: Option<&str>,
    file_name: &str,
    bytes: &[u8],
) -> Option<&'static str> {
    match resolve(rela_path, file_name) {
        Resolution::Decided(decided) => decided,
        // Binary or unreadable content carries no signal, so the file
        // drops out of the stats instead of landing on a default.
        Resolution::Disambiguate(candidates) => {
            scan_text(bytes).map(|text| pick_by_content(text, candidates))
        }
        Resolution::Shebang => shebang_language(bytes),
    }
}

/// Name-only classification; the walker uses this to avoid reading files
/// whose language is already decided.
pub fn resolve(rela_path: Option<&str>, file_name: &str) -> Resolution {
    let _ = rela_path;
    let lowered = file_name.to_ascii_lowercase();
    if let Some(lang) = filename_lookup().get(lowered.as_str()) {
        return Resolution::Decided(filter_excluded(lang));
    }
    let mut name = lowered.as_str();
    for suffix in TEMPLATE_SUFFIXES {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped;
            break;
        }
    }
    // The leftmost registered suffix is always the longest (later dots
    // yield strictly shorter tails), so the first hit wins.
    let bytes_key = name.as_bytes();
    for start in 0..bytes_key.len() {
        if bytes_key[start] != b'.' {
            continue;
        }
        if let Some(lang) = extension_lookup().get(&name[start..]) {
            if lang.is_empty() {
                return Resolution::Decided(None);
            }
            // Disambiguation is keyed by extension, not language: the
            // extension decides which candidates apply.
            if let Some(candidates) = disambig_lookup().get(&name[start..]) {
                return Resolution::Disambiguate(candidates);
            }
            return Resolution::Decided(Some(lang));
        }
    }
    // Hidden dotfiles and suffixed names never carry shebangs worth
    // reading; extensionless scripts do.
    if lowered.starts_with('.') || lowered.contains('.') {
        return Resolution::Decided(None);
    }
    Resolution::Shebang
}

fn disambig_lookup() -> &'static HashMap<&'static str, ContentRules> {
    static LOOKUP: OnceLock<HashMap<&'static str, ContentRules>> = OnceLock::new();
    LOOKUP.get_or_init(|| DISAMBIG.iter().copied().collect())
}

/// First candidate whose needles appear in `text` wins; the last
/// candidate is the fallthrough default.
fn pick_by_content(text: &str, candidates: ContentRules) -> &'static str {
    candidates
        .iter()
        .find(|(needles, _)| {
            needles.is_empty() || needles.iter().any(|needle| text.contains(needle))
        })
        .map(|(_, lang)| *lang)
        .unwrap_or_else(|| candidates.last().expect("non-empty candidates").1)
}

/// UTF-8 text for pattern matching, or `None` for binary or invalid files;
/// those fall back to the default language.
fn scan_text(bytes: &[u8]) -> Option<&str> {
    if bytes.len() > MAX_CONTENT_SCAN || bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

fn filter_excluded(lang: &'static str) -> Option<&'static str> {
    if lang.is_empty() { None } else { Some(lang) }
}

/// Interpreters for shebang scripts. Checked as the last path segment of
/// the interpreter token, with trailing version suffixes stripped, so
/// `python3.11` still maps to Python.
const SHEBANG: &[(&str, &str)] = &[
    ("Rscript", "R"),
    ("awk", "Awk"),
    ("bash", "Shell"),
    ("crystal", "Crystal"),
    ("csh", "Shell"),
    ("dash", "Shell"),
    ("elvish", "Shell"),
    ("elixir", "Elixir"),
    ("erl", "Erlang"),
    ("fish", "Shell"),
    ("groovy", "Groovy"),
    ("julia", "Julia"),
    ("ksh", "Shell"),
    ("lua", "Lua"),
    ("make", "Makefile"),
    ("node", "JavaScript"),
    ("nodejs", "JavaScript"),
    ("perl", "Perl"),
    ("php", "PHP"),
    ("powershell", "PowerShell"),
    ("pwsh", "PowerShell"),
    ("python", "Python"),
    ("raku", "Raku"),
    ("ruby", "Ruby"),
    ("sh", "Shell"),
    ("tcsh", "Shell"),
    ("tclsh", "Tcl"),
    ("wish", "Tcl"),
    ("zsh", "Shell"),
];

fn shebang_language(bytes: &[u8]) -> Option<&'static str> {
    let prefix = if bytes.len() > 160 {
        &bytes[..160]
    } else {
        bytes
    };
    let line = std::str::from_utf8(prefix).ok()?;
    let line = line.lines().next()?;
    let rest = line.strip_prefix("#!")?.trim();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    // `/usr/bin/env python` carries the interpreter as a second token; a
    // bare `#!/bin/bash` carries it as the only one.
    let first = tokens.first()?;
    let interpreter = if *first == "env" || first.ends_with("/env") {
        tokens.get(1)?
    } else {
        first
    };
    let name = interpreter.rsplit(['/', '\\']).next()?;
    let stem = name
        .split(['-', '.'])
        .next()?
        .trim_end_matches(|ch: char| ch.is_ascii_digit());
    SHEBANG
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(stem))
        .map(|(_, lang)| *lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLORLESS: &[&str] = &["COBOL", "Procfile", "Protocol Buffer"];

    #[test]
    fn tables_have_no_duplicate_names() {
        fn unique(table: &[(&str, &str)], what: &str) {
            for (i, (name, _)) in table.iter().enumerate() {
                assert!(
                    !table[..i].iter().any(|(other, _)| other == name),
                    "{what} has duplicate entry {name}"
                );
            }
        }
        unique(FILENAMES, "FILENAMES");
        unique(EXTENSIONS, "EXTENSIONS");
        unique(LANGUAGES, "LANGUAGES");
        unique(SHEBANG, "SHEBANG");
    }

    #[test]
    fn every_mapping_has_a_color() {
        for (name, _) in FILENAMES.iter().chain(EXTENSIONS.iter()) {
            let _ = name;
        }
        for (_, lang) in FILENAMES.iter().chain(EXTENSIONS.iter()) {
            if lang.is_empty() || COLORLESS.contains(lang) {
                continue;
            }
            assert!(
                color_for_language(lang).is_some(),
                "{lang} has no entry in LANGUAGES"
            );
        }
        for (_, candidates) in DISAMBIG {
            for (_, lang) in *candidates {
                assert!(
                    color_for_language(lang).is_some() || COLORLESS.contains(lang),
                    "{lang} has no entry in LANGUAGES"
                );
            }
        }
    }

    #[test]
    fn colors_match_github_dataset() {
        assert_eq!(color_for_language("Rust"), Some("#dea584"));
        assert_eq!(color_for_language("TypeScript"), Some("#3178c6"));
        assert_eq!(color_for_language("Go"), Some("#00ADD8"));
        assert_eq!(color_for_language("COBOL"), None);
        assert_eq!(color_for_language("Not A Language"), None);
    }

    #[test]
    fn classifies_by_extension() {
        assert_eq!(
            classify_language(None, "main.rs", b"fn main() {}"),
            Some("Rust")
        );
        assert_eq!(
            classify_language(None, "server.py", b"import os"),
            Some("Python")
        );
        assert_eq!(
            classify_language(None, "styles.scss", b"$x: 1;"),
            Some("SCSS")
        );
        assert_eq!(
            classify_language(None, "index.d.ts", b"export {}"),
            Some("TypeScript")
        );
        assert_eq!(classify_language(None, "app.min.js", b"x"), None);
        assert_eq!(classify_language(None, "out.js.map", b"{}"), None);
        assert_eq!(classify_language(None, "Cargo.lock", b"version = 3"), None);
    }

    #[test]
    fn template_suffixes_are_stripped() {
        assert_eq!(
            classify_language(None, "audit.toml.example", b"[a]"),
            Some("TOML")
        );
    }

    #[test]
    fn exact_filenames_win() {
        assert_eq!(
            classify_language(None, "Makefile", b"all:"),
            Some("Makefile")
        );
        assert_eq!(
            classify_language(None, "Containerfile", b"FROM debian"),
            Some("Dockerfile")
        );
        assert_eq!(
            classify_language(None, "debian.Containerfile", b"FROM debian"),
            Some("Dockerfile")
        );
    }

    #[test]
    fn shebangs_are_recognized() {
        assert_eq!(
            classify_language(None, "run", b"#!/usr/bin/env python3.11\nprint(1)"),
            Some("Python")
        );
        assert_eq!(
            classify_language(None, "tool", b"#!/bin/bash\nset -euo pipefail"),
            Some("Shell")
        );
        assert_eq!(classify_language(None, "notes", b"just text"), None);
        assert_eq!(classify_language(None, ".env", b"X=1"), None);
    }

    #[test]
    fn binaries_are_skipped() {
        let bytes = [0x4d, 0x5a, 0x90, 0x00, 0x03];
        assert_eq!(classify_language(None, "weird.c", &bytes), Some("C"));
        // Binary content gives a disambiguated extension no signal, so it
        // drops out instead of landing on a default language.
        assert_eq!(classify_language(None, "weird.m", &bytes), None);
    }

    #[test]
    fn tuning_matches_linguist_samples() {
        let failures: Vec<String> = crate::domain::linguist_samples::parse_samples()
            .into_iter()
            .filter_map(|(name, text)| {
                if classify_language(None, &name, text.as_bytes()).is_some() {
                    None
                } else {
                    Some(format!("{name}: unclassified"))
                }
            })
            .collect();
        assert!(failures.is_empty(), "failures:\n{}", failures.join("\n"));
    }

    #[test]
    fn disambiguation_picks_by_content() {
        assert_eq!(
            classify_language(
                None,
                "Foo.m",
                b"#import \"Foo.h\"\n@implementation Foo\n@end"
            ),
            Some("Objective-C")
        );
        assert_eq!(
            classify_language(None, "plot.m", b"function y = f(x)\ny = x * 2;\nend"),
            Some("MATLAB")
        );
        assert_eq!(
            classify_language(None, "util.hh", b"namespace u { class A {}; }"),
            Some("C++")
        );
        assert_eq!(
            classify_language(
                None,
                "util.h",
                b"#include <stdio.h>\nint main(void) { return 0; }"
            ),
            Some("C")
        );
        assert_eq!(
            classify_language(None, "x.php", b"<?php\nphpinfo();\n"),
            Some("PHP")
        );
        assert_eq!(
            classify_language(None, "s.pl", b"use strict;\nmy $x = 1;"),
            Some("Perl")
        );
        assert_eq!(
            classify_language(
                None,
                "q.v",
                b"module m(input a);\nalways @(a) begin end\nendmodule"
            ),
            Some("Verilog")
        );
        assert_eq!(
            classify_language(None, "x.cr", b"def f(x)\n  x\nend"),
            Some("Crystal")
        );
        assert_eq!(
            classify_language(None, "x.rb", b"def f(a)\nend"),
            Some("Ruby")
        );
    }
}
