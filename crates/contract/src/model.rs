use std::collections::HashMap;

use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprLit, Lit, Meta, Token};

pub fn ident_string(ident: &syn::Ident) -> String {
    ident.unraw().to_string()
}

#[derive(Debug, Default, Clone)]
pub struct SerdeAttrs {
    pub rename: Option<String>,
    pub rename_all: Option<String>,
    pub rename_all_fields: Option<String>,
    pub tag: Option<String>,
    pub content: Option<String>,
    pub untagged: bool,
    pub transparent: bool,
    pub default: bool,
    pub skip: bool,
    pub skip_serializing: bool,
    pub skip_serializing_if: Option<String>,
    pub flatten: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: Option<String>,
    pub ty: syn::Type,
    pub serde: SerdeAttrs,
    pub docs: String,
}

#[derive(Debug, Clone)]
pub enum Fields {
    Named(Vec<Field>),
    Unnamed(Vec<Field>),
    Unit,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Fields,
    pub serde: SerdeAttrs,
    pub docs: String,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Struct(Fields),
    Enum(Vec<Variant>),
    Alias(syn::Type),
}

#[derive(Debug, Clone)]
pub struct TypeItem {
    pub fqname: String,
    pub name: String,
    pub module: String,
    pub kind: ItemKind,
    pub serde: SerdeAttrs,
    pub docs: String,
    pub derives_serialize: bool,
    pub manual_serialize: bool,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub module: String,
    pub args: Vec<CommandArg>,
    pub result: syn::Type,
    pub docs: String,
}

#[derive(Debug, Clone)]
pub struct CommandArg {
    pub name: String,
    pub ty: syn::Type,
    pub optional: bool,
}

#[derive(Debug, Default)]
pub struct Workspace {
    pub items: HashMap<String, TypeItem>,
    pub aliases: HashMap<String, String>,
    pub by_name: HashMap<String, Vec<String>>,
    pub modules: HashMap<String, ModuleCtx>,
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleCtx {
    pub crate_name: String,
    pub module: String,
    pub imports: HashMap<String, String>,
    pub glob_imports: Vec<String>,
}

impl Workspace {
    pub fn add_item(&mut self, item: TypeItem) {
        self.by_name
            .entry(item.name.clone())
            .or_default()
            .push(item.fqname.clone());
        self.items.insert(item.fqname.clone(), item);
    }

    pub fn insert_import(&mut self, module: &str, alias: &str, target: &str) {
        if let Some(ctx) = self.modules.get_mut(module) {
            ctx.imports.insert(alias.to_owned(), target.to_owned());
        }
    }

    pub fn insert_glob(&mut self, module: &str, target: &str) {
        if let Some(ctx) = self.modules.get_mut(module) {
            ctx.glob_imports.push(target.to_owned());
        }
    }

    pub fn resolve(&self, fqname: &str) -> Option<&TypeItem> {
        if let Some(item) = self.items.get(fqname) {
            return Some(item);
        }
        let mut current = fqname;
        let mut seen = 0;
        loop {
            let next = self.aliases.get(current)?;
            if seen > 32 {
                return None;
            }
            if let Some(item) = self.items.get(next) {
                return Some(item);
            }
            current = next;
            seen += 1;
        }
    }

    pub fn resolve_path(&self, module: &str, segments: &[String]) -> Option<String> {
        if segments.is_empty() {
            return None;
        }
        let ctx = self.modules.get(module);
        if segments.len() == 1 {
            return self.resolve_name(&segments[0], module);
        }

        let mut parts: Vec<String> = Vec::new();
        let mut index = 0;
        match segments[0].as_str() {
            "crate" => {
                if let Some(ctx) = ctx {
                    parts.push(ctx.crate_name.clone());
                }
                index = 1;
            }
            "self" => {
                if let Some(ctx) = ctx {
                    parts.extend(ctx.module.split("::").map(ToString::to_string));
                }
                index = 1;
            }
            "super" => {
                let mut parent = ctx
                    .map(|ctx| {
                        ctx.module
                            .split("::")
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                while index < segments.len() && segments[index] == "super" {
                    parent.pop();
                    index += 1;
                }
                parts.extend(parent);
            }
            first => {
                if self.items.keys().any(|key| key == first) || self.modules.contains_key(first) {
                    parts.push(first.to_owned());
                    index = 1;
                } else if let Some(ctx) = ctx {
                    if let Some(target) = ctx.imports.get(first) {
                        parts.extend(target.split("::").map(ToString::to_string));
                        index = 1;
                    } else {
                        let relative = format!("{}::{}", ctx.module, segments.join("::"));
                        if self.items.contains_key(&relative)
                            || self.aliases.contains_key(&relative)
                        {
                            return Some(relative);
                        }
                        return self.resolve_name(segments.last()?, module);
                    }
                } else {
                    return self.resolve_name(segments.last()?, module);
                }
            }
        }
        parts.extend(segments[index..].iter().cloned());
        let candidate = parts.join("::");
        if self.items.contains_key(&candidate) || self.aliases.contains_key(&candidate) {
            return Some(candidate);
        }
        self.resolve_name(segments.last()?, module)
    }

    pub fn resolve_name(&self, name: &str, module: &str) -> Option<String> {
        let ctx = self.modules.get(module);
        if let Some(ctx) = ctx {
            if let Some(target) = ctx.imports.get(name) {
                return Some(target.clone());
            }
            let local = format!("{module}::{name}");
            if self.items.contains_key(&local) || self.aliases.contains_key(&local) {
                return Some(local);
            }
            for glob in &ctx.glob_imports {
                let candidate = format!("{glob}::{name}");
                if self.items.contains_key(&candidate) || self.aliases.contains_key(&candidate) {
                    return Some(candidate);
                }
            }
        }
        let candidates = self.by_name.get(name)?;
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }
        candidates
            .iter()
            .find(|candidate| candidate.starts_with(&format!("{module}::")))
            .cloned()
            .or_else(|| candidates.first().cloned())
    }
}

pub fn docs(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let Meta::NameValue(value) = &attr.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) = &value.value
        else {
            continue;
        };
        lines.push(text.value().trim().to_owned());
    }
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

pub fn serde_attrs(attrs: &[Attribute]) -> SerdeAttrs {
    let mut out = SerdeAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let nested = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .unwrap_or_default();
        for meta in nested {
            match meta {
                Meta::Path(path) => {
                    if path.is_ident("untagged") {
                        out.untagged = true;
                    } else if path.is_ident("transparent") {
                        out.transparent = true;
                    } else if path.is_ident("default") {
                        out.default = true;
                    } else if path.is_ident("skip") {
                        out.skip = true;
                    } else if path.is_ident("skip_serializing") {
                        out.skip_serializing = true;
                    } else if path.is_ident("flatten") {
                        out.flatten = true;
                    }
                }
                Meta::NameValue(value) => {
                    let Some(key) = value.path.get_ident().map(ToString::to_string) else {
                        continue;
                    };
                    let Expr::Lit(ExprLit {
                        lit: Lit::Str(text),
                        ..
                    }) = &value.value
                    else {
                        continue;
                    };
                    let text = text.value();
                    match key.as_str() {
                        "rename" => out.rename = Some(text),
                        "rename_all" => out.rename_all = Some(text),
                        "rename_all_fields" => out.rename_all_fields = Some(text),
                        "tag" => out.tag = Some(text),
                        "content" => out.content = Some(text),
                        "skip_serializing_if" => out.skip_serializing_if = Some(text),
                        _ => {}
                    }
                }
                Meta::List(_) => {}
            }
        }
    }
    out
}

pub fn has_derive(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let Ok(paths) = attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
        else {
            return false;
        };
        paths.iter().any(|path| {
            path.segments
                .last()
                .is_some_and(|segment| segment.ident == name)
        })
    })
}

pub fn rename(name: &str, rule: Option<&str>) -> String {
    let Some(rule) = rule else {
        return name.to_owned();
    };
    match rule {
        "camelCase" => to_camel_case(name),
        "lowercase" => name.to_lowercase(),
        "UPPERCASE" => name.to_uppercase(),
        "PascalCase" => to_pascal_case(name),
        "snake_case" => to_snake_case(name),
        "SCREAMING_SNAKE_CASE" => to_snake_case(name).to_uppercase(),
        "kebab-case" => to_snake_case(name).replace('_', "-"),
        "SCREAMING-KEBAB-CASE" => to_snake_case(name).to_uppercase().replace('_', "-"),
        _ => name.to_owned(),
    }
}

pub fn to_camel_case(name: &str) -> String {
    let pascal = to_pascal_case(name);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn to_pascal_case(name: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in name.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else if ch == '-' {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out
}
