//! Embedded scaffolding content for repository creation: .gitignore presets
//! and permissive license texts, stored under `crates/git-backend/assets`.
//! Everything is static so dialogs work offline; the lists exposed over IPC
//! are exactly the ids matched here.

pub const GITIGNORE_TEMPLATES: &[GitignoreTemplate] = &[
    GitignoreTemplate {
        id: "node",
        label: "Node",
        body: include_str!("../../assets/gitignore/node.gitignore"),
    },
    GitignoreTemplate {
        id: "rust",
        label: "Rust",
        body: include_str!("../../assets/gitignore/rust.gitignore"),
    },
    GitignoreTemplate {
        id: "python",
        label: "Python",
        body: include_str!("../../assets/gitignore/python.gitignore"),
    },
    GitignoreTemplate {
        id: "go",
        label: "Go",
        body: include_str!("../../assets/gitignore/go.gitignore"),
    },
    GitignoreTemplate {
        id: "java",
        label: "Java",
        body: include_str!("../../assets/gitignore/java.gitignore"),
    },
    GitignoreTemplate {
        id: "csharp",
        label: "C#",
        body: include_str!("../../assets/gitignore/csharp.gitignore"),
    },
    GitignoreTemplate {
        id: "cpp",
        label: "C++",
        body: include_str!("../../assets/gitignore/cpp.gitignore"),
    },
    GitignoreTemplate {
        id: "swift",
        label: "Swift",
        body: include_str!("../../assets/gitignore/swift.gitignore"),
    },
    GitignoreTemplate {
        id: "kotlin",
        label: "Kotlin",
        body: include_str!("../../assets/gitignore/kotlin.gitignore"),
    },
    GitignoreTemplate {
        id: "ruby",
        label: "Ruby",
        body: include_str!("../../assets/gitignore/ruby.gitignore"),
    },
    GitignoreTemplate {
        id: "php",
        label: "PHP (Laravel)",
        body: include_str!("../../assets/gitignore/php-laravel.gitignore"),
    },
    GitignoreTemplate {
        id: "dotnet",
        label: ".NET",
        body: include_str!("../../assets/gitignore/dotnet.gitignore"),
    },
    GitignoreTemplate {
        id: "android",
        label: "Android",
        body: include_str!("../../assets/gitignore/android.gitignore"),
    },
    GitignoreTemplate {
        id: "elixir",
        label: "Elixir",
        body: include_str!("../../assets/gitignore/elixir.gitignore"),
    },
    GitignoreTemplate {
        id: "haskell",
        label: "Haskell",
        body: include_str!("../../assets/gitignore/haskell.gitignore"),
    },
    GitignoreTemplate {
        id: "macos",
        label: "macOS",
        body: include_str!("../../assets/gitignore/macos.gitignore"),
    },
    GitignoreTemplate {
        id: "windows",
        label: "Windows",
        body: include_str!("../../assets/gitignore/windows.gitignore"),
    },
    GitignoreTemplate {
        id: "linux",
        label: "Linux",
        body: include_str!("../../assets/gitignore/linux.gitignore"),
    },
    GitignoreTemplate {
        id: "jetbrains",
        label: "JetBrains IDEs",
        body: include_str!("../../assets/gitignore/jetbrains.gitignore"),
    },
    GitignoreTemplate {
        id: "vscode",
        label: "Visual Studio Code",
        body: include_str!("../../assets/gitignore/vscode.gitignore"),
    },
    GitignoreTemplate {
        id: "vim",
        label: "Vim",
        body: include_str!("../../assets/gitignore/vim.gitignore"),
    },
];

pub const LICENSE_TEMPLATES: &[LicenseTemplate] = &[
    LicenseTemplate {
        id: "mit",
        name: "MIT License",
        description: "Permissive, keep copyright and license notice",
        copyright_line: Some("Copyright (c) {year} {holder}"),
        body: include_str!("../../assets/license/mit.txt"),
    },
    LicenseTemplate {
        id: "apache-2.0",
        name: "Apache License 2.0",
        description: "Permissive, keep notice, state changes, patent grant",
        copyright_line: Some("Copyright {year} {holder}"),
        body: include_str!("../../assets/license/apache-2.0.txt"),
    },
    LicenseTemplate {
        id: "bsd-3-clause",
        name: "BSD 3-Clause License",
        description: "Permissive, keep notice, no endorsement with names",
        copyright_line: Some("Copyright (c) {year} {holder}"),
        body: include_str!("../../assets/license/bsd-3-clause.txt"),
    },
    LicenseTemplate {
        id: "mpl-2.0",
        name: "Mozilla Public License 2.0",
        description: "Weak copyleft, share changed MPL files with source",
        copyright_line: None,
        body: include_str!("../../assets/license/mpl-2.0.txt"),
    },
    LicenseTemplate {
        id: "unlicense",
        name: "The Unlicense",
        description: "No conditions, public domain where allowed",
        copyright_line: None,
        body: include_str!("../../assets/license/unlicense.txt"),
    },
];

pub struct GitignoreTemplate {
    pub id: &'static str,
    pub label: &'static str,
    pub body: &'static str,
}

pub struct LicenseTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Present when the license expects a copyright notice filled in;
    /// `{year}` and `{holder}` are replaced at write time.
    pub copyright_line: Option<&'static str>,
    pub body: &'static str,
}

pub fn gitignore_by_id(id: &str) -> Option<&'static GitignoreTemplate> {
    GITIGNORE_TEMPLATES.iter().find(|t| t.id == id)
}

pub fn license_by_id(id: &str) -> Option<&'static LicenseTemplate> {
    LICENSE_TEMPLATES.iter().find(|t| t.id == id)
}

/// Renders license text with the copyright line (when the template expects
/// one) inserted directly above the body.
pub fn render_license(template: &LicenseTemplate, year: i32, holder: &str) -> String {
    match template.copyright_line {
        None => template.body.to_string(),
        Some(line) => {
            let filled = line.replace("{year}", &year.to_string()).replace(
                "{holder}",
                if holder.trim().is_empty() {
                    "[fullname]"
                } else {
                    holder
                },
            );
            format!("{filled}\n\n{}", template.body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for t in GITIGNORE_TEMPLATES {
            assert!(seen.insert(t.id), "duplicate gitignore id {}", t.id);
        }
        for t in LICENSE_TEMPLATES {
            assert!(seen.insert(t.id), "duplicate license id {}", t.id);
        }
    }

    #[test]
    fn lookup_finds_and_renders() {
        assert!(gitignore_by_id("rust").is_some());
        assert!(gitignore_by_id("nope").is_none());
        let mit = license_by_id("mit").unwrap();
        let text = render_license(mit, 2026, "Ada");
        assert!(text.starts_with("Copyright (c) 2026 Ada\n"));
        assert!(text.contains("Permission is hereby granted"));
    }
}
