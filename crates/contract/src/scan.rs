use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::{
    Fields as SynFields, FnArg, Item, ItemEnum, ItemFn, ItemImpl, ItemMod, ItemStruct, ItemType,
    ItemUse, Meta, Pat, ReturnType, Token, Type, UseTree,
};

use crate::model::{
    docs, has_derive, ident_string, serde_attrs, to_camel_case, Command, CommandArg, Field, Fields,
    ItemKind, ModuleCtx, TypeItem, Variant, Workspace,
};

const SKIP_DIRS: &[&str] = &["bin", "tests", "benches", "examples"];

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("contract crate lives under crates/")
        .to_path_buf()
}

pub fn load(root: &Path) -> Result<Workspace, String> {
    let mut files = Vec::new();
    let crates_dir = root.join("crates");
    let entries = fs::read_dir(&crates_dir)
        .map_err(|error| format!("read {}: {error}", crates_dir.display()))?;
    let mut crate_roots = HashSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            continue;
        }
        let package = entry.file_name().to_string_lossy().to_string();
        let crate_name = package.replace('-', "_");
        crate_roots.insert(crate_name.clone());
        let src = entry.path().join("src");
        if src.is_dir() {
            collect_files(&src, &src, &crate_name, &mut files)?;
        }
    }

    let mut parsed = BTreeMap::new();
    for (module, path) in &files {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let ast = syn::parse_file(&source)
            .map_err(|error| format!("parse {}: {error}", path.display()))?;
        parsed.insert(module.clone(), ast);
    }

    let mut ws = Workspace::default();
    for module in parsed.keys() {
        let crate_name = module.split("::").next().unwrap_or_default().to_owned();
        ws.modules.insert(
            module.clone(),
            ModuleCtx {
                crate_name,
                module: module.clone(),
                ..ModuleCtx::default()
            },
        );
    }
    let mut manual_serialize = Vec::new();
    for (module, ast) in &parsed {
        let Some(ctx) = ws.modules.get(module).cloned() else {
            continue;
        };
        collect_items(
            &mut ws,
            module,
            &ctx,
            &ast.items,
            &crate_roots,
            &mut manual_serialize,
        )?;
    }

    for (module, name) in manual_serialize {
        let fqname = format!("{module}::{name}");
        if let Some(item) = ws.items.get_mut(&fqname) {
            item.manual_serialize = true;
        }
    }

    ws.commands
        .sort_by(|left, right| left.name.cmp(&right.name));
    Ok(ws)
}

fn collect_files(
    root: &Path,
    dir: &Path,
    crate_name: &str,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            if SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            collect_files(root, &path, crate_name, out)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
        let mut components = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let Some(file_name) = components.pop() else {
            continue;
        };
        if file_name == "main.rs" {
            continue;
        }
        let stem = file_name.trim_end_matches(".rs");
        if stem != "lib" && stem != "mod" {
            components.push(stem.to_owned());
        }
        let mut module = vec![crate_name.to_owned()];
        module.extend(components);
        out.push((module.join("::"), path));
    }
    Ok(())
}

fn collect_items(
    ws: &mut Workspace,
    module: &str,
    ctx: &ModuleCtx,
    items: &[Item],
    crate_roots: &HashSet<String>,
    manual_serialize: &mut Vec<(String, String)>,
) -> Result<(), String> {
    for item in items {
        match item {
            Item::Struct(item) => collect_struct(ws, module, item),
            Item::Enum(item) => collect_enum(ws, module, item),
            Item::Type(item) => collect_alias(ws, module, item),
            Item::Macro(item) => collect_macro(ws, module, item),
            Item::Use(item) => collect_use(ws, module, ctx, item, crate_roots),
            Item::Mod(item) => {
                collect_inline_mod(ws, module, ctx, item, crate_roots, manual_serialize)?
            }
            Item::Impl(item) => collect_impl(module, item, manual_serialize),
            Item::Fn(item) => collect_command(ws, module, ctx, item, crate_roots),
            _ => {}
        }
    }
    Ok(())
}

fn collect_inline_mod(
    ws: &mut Workspace,
    parent: &str,
    parent_ctx: &ModuleCtx,
    item: &ItemMod,
    crate_roots: &HashSet<String>,
    manual_serialize: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let Some((_, items)) = &item.content else {
        return Ok(());
    };
    if item.ident == "tests" || is_cfg_test(&item.attrs) {
        return Ok(());
    }
    let module = format!("{parent}::{}", item.ident);
    let ctx = ModuleCtx {
        crate_name: parent_ctx.crate_name.clone(),
        module: module.clone(),
        ..ModuleCtx::default()
    };
    ws.modules.insert(module.clone(), ctx.clone());
    collect_items(ws, &module, &ctx, items, crate_roots, manual_serialize)
}

fn collect_struct(ws: &mut Workspace, module: &str, item: &ItemStruct) {
    let fields = collect_fields(&item.fields);
    let fqname = format!("{module}::{}", item.ident);
    ws.add_item(TypeItem {
        fqname,
        name: ident_string(&item.ident),
        module: module.to_owned(),
        kind: ItemKind::Struct(fields),
        serde: serde_attrs(&item.attrs),
        docs: docs(&item.attrs),
        derives_serialize: has_derive(&item.attrs, "Serialize"),
        manual_serialize: false,
    });
}

fn collect_enum(ws: &mut Workspace, module: &str, item: &ItemEnum) {
    let variants = item
        .variants
        .iter()
        .map(|variant| Variant {
            name: ident_string(&variant.ident),
            fields: collect_fields(&variant.fields),
            serde: serde_attrs(&variant.attrs),
            docs: docs(&variant.attrs),
        })
        .collect();
    let fqname = format!("{module}::{}", item.ident);
    ws.add_item(TypeItem {
        fqname,
        name: ident_string(&item.ident),
        module: module.to_owned(),
        kind: ItemKind::Enum(variants),
        serde: serde_attrs(&item.attrs),
        docs: docs(&item.attrs),
        derives_serialize: has_derive(&item.attrs, "Serialize"),
        manual_serialize: false,
    });
}

fn collect_macro(ws: &mut Workspace, module: &str, item: &syn::ItemMacro) {
    use syn::parse::Parser;

    if !item.mac.path.is_ident("id_newtype") {
        return;
    }
    let parser = |input: syn::parse::ParseStream| {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let name: syn::Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let ty: Type = input.parse()?;
        Ok((attrs, name, ty))
    };
    let Ok((inner_attrs, name, ty)) = parser.parse2(item.mac.tokens.clone()) else {
        return;
    };
    let fqname = format!("{module}::{name}");
    let mut merged = item.attrs.clone();
    merged.extend(inner_attrs);
    ws.add_item(TypeItem {
        fqname,
        name: ident_string(&name),
        module: module.to_owned(),
        kind: ItemKind::Alias(ty),
        serde: crate::model::SerdeAttrs {
            transparent: true,
            ..crate::model::SerdeAttrs::default()
        },
        docs: docs(&merged),
        derives_serialize: true,
        manual_serialize: false,
    });
}

fn collect_alias(ws: &mut Workspace, module: &str, item: &ItemType) {
    let fqname = format!("{module}::{}", item.ident);
    ws.add_item(TypeItem {
        fqname,
        name: ident_string(&item.ident),
        module: module.to_owned(),
        kind: ItemKind::Alias((*item.ty).clone()),
        serde: serde_attrs(&item.attrs),
        docs: docs(&item.attrs),
        derives_serialize: false,
        manual_serialize: false,
    });
}

fn collect_fields(fields: &SynFields) -> Fields {
    match fields {
        SynFields::Named(named) => Fields::Named(
            named
                .named
                .iter()
                .map(|field| Field {
                    name: field.ident.as_ref().map(ident_string),
                    ty: field.ty.clone(),
                    serde: serde_attrs(&field.attrs),
                    docs: docs(&field.attrs),
                })
                .collect(),
        ),
        SynFields::Unnamed(unnamed) => Fields::Unnamed(
            unnamed
                .unnamed
                .iter()
                .map(|field| Field {
                    name: None,
                    ty: field.ty.clone(),
                    serde: serde_attrs(&field.attrs),
                    docs: docs(&field.attrs),
                })
                .collect(),
        ),
        SynFields::Unit => Fields::Unit,
    }
}

fn collect_use(
    ws: &mut Workspace,
    module: &str,
    ctx: &ModuleCtx,
    item: &ItemUse,
    crate_roots: &HashSet<String>,
) {
    let is_public = matches!(
        item.vis,
        syn::Visibility::Public(_) | syn::Visibility::Restricted(_)
    );
    let mut entries = Vec::new();
    flatten_use_tree(&item.tree, Vec::new(), &mut entries);
    for (prefix, name, alias) in entries {
        let Some(mut target) = resolve_segments(&prefix, ctx, crate_roots) else {
            continue;
        };
        if name == "*" {
            ws.insert_glob(module, &target);
            continue;
        }
        target.push_str("::");
        target.push_str(&name);
        let local = format!("{module}::{alias}");
        if is_public {
            ws.aliases.insert(local.clone(), target.clone());
        }
        ws.insert_import(module, &alias, &target);
    }
}

fn flatten_use_tree(
    tree: &UseTree,
    prefix: Vec<String>,
    out: &mut Vec<(Vec<String>, String, String)>,
) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            flatten_use_tree(&path.tree, next, out);
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident == "self" {
                let alias = prefix.last().cloned().unwrap_or_default();
                out.push((prefix, alias.clone(), alias));
            } else {
                out.push((prefix, ident.clone(), ident));
            }
        }
        UseTree::Rename(rename) => {
            out.push((prefix, rename.ident.to_string(), rename.rename.to_string()));
        }
        UseTree::Glob(_) => out.push((prefix, "*".to_owned(), "*".to_owned())),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(item, prefix.clone(), out);
            }
        }
    }
}

fn collect_impl(module: &str, item: &ItemImpl, manual_serialize: &mut Vec<(String, String)>) {
    let Some((_, trait_path, _)) = &item.trait_ else {
        return;
    };
    let is_serialize = trait_path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Serialize");
    if !is_serialize {
        return;
    }
    if let Type::Path(path) = &*item.self_ty {
        if let Some(segment) = path.path.segments.last() {
            manual_serialize.push((module.to_owned(), segment.ident.to_string()));
        }
    }
}

fn collect_command(
    ws: &mut Workspace,
    module: &str,
    ctx: &ModuleCtx,
    item: &ItemFn,
    crate_roots: &HashSet<String>,
) {
    if !item.attrs.iter().any(is_tauri_command) {
        return;
    }

    let (wire_name, snake_args) = command_options(&item.attrs, &item.sig.ident);
    let mut args = Vec::new();
    for input in &item.sig.inputs {
        let FnArg::Typed(pat) = input else {
            continue;
        };
        let Some(name) = pattern_name(&pat.pat) else {
            continue;
        };
        let ty = (*pat.ty).clone();
        if is_injected_type(&ty) {
            continue;
        }
        let wire = if snake_args {
            name.clone()
        } else {
            to_camel_case(&name)
        };
        args.push(CommandArg {
            name: wire,
            optional: is_option(&ty),
            ty,
        });
    }

    let result = match &item.sig.output {
        ReturnType::Default => syn::parse_quote!(()),
        ReturnType::Type(_, ty) => unwrap_result(ty),
    };

    let _ = (ctx, crate_roots);
    ws.commands.push(Command {
        name: wire_name,
        module: module.to_owned(),
        args,
        result,
        docs: docs(&item.attrs),
    });
}

fn command_options(attrs: &[syn::Attribute], ident: &syn::Ident) -> (String, bool) {
    let mut name = ident_string(ident);
    let mut snake = false;
    for attr in attrs {
        if !is_tauri_command(attr) {
            continue;
        }
        let Ok(metas) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            continue;
        };
        for meta in metas {
            let Meta::NameValue(value) = meta else {
                continue;
            };
            let Some(key) = value.path.get_ident().map(ToString::to_string) else {
                continue;
            };
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) = value.value
            else {
                continue;
            };
            match key.as_str() {
                "rename" => name = text.value(),
                "rename_all" if text.value() == "snake_case" => snake = true,
                _ => {}
            }
        }
    }
    (name, snake)
}

fn is_tauri_command(attr: &syn::Attribute) -> bool {
    let segments = &attr.path().segments;
    segments.len() >= 2
        && segments
            .first()
            .is_some_and(|segment| segment.ident == "tauri")
        && segments
            .last()
            .is_some_and(|segment| segment.ident == "command")
}

fn is_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .map(|metas| metas.iter().any(|meta| meta.path().is_ident("test")))
            .unwrap_or(false)
    })
}

fn pattern_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) => Some(ident_string(&ident.ident)),
        Pat::Wild(_) => None,
        _ => None,
    }
}

fn is_injected_type(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    matches!(
        segment.ident.to_string().as_str(),
        "SharedState"
            | "State"
            | "AppHandle"
            | "Window"
            | "WebviewWindow"
            | "Webview"
            | "Ipc"
            | "Request"
    )
}

fn is_option(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(path)
            if path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "Option")
    )
}

fn unwrap_result(ty: &Type) -> Type {
    let Type::Path(path) = ty else {
        return ty.clone();
    };
    let Some(segment) = path.path.segments.last() else {
        return ty.clone();
    };
    if !matches!(
        segment.ident.to_string().as_str(),
        "Result" | "CommandResult"
    ) {
        return ty.clone();
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return ty.clone();
    };
    args.args
        .iter()
        .find_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .unwrap_or_else(|| ty.clone())
}

fn resolve_segments(
    segments: &[String],
    ctx: &ModuleCtx,
    crate_roots: &HashSet<String>,
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    let mut index = 0;
    match segments[0].as_str() {
        "crate" => {
            out.push(ctx.crate_name.clone());
            index = 1;
        }
        "self" => {
            out.extend(ctx.module.split("::").map(ToString::to_string));
            index = 1;
        }
        "super" => {
            let mut parts = ctx
                .module
                .split("::")
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            while index < segments.len() && segments[index] == "super" {
                parts.pop();
                index += 1;
            }
            out.extend(parts);
        }
        first if crate_roots.contains(first) => {
            out.push(first.to_owned());
            index = 1;
        }
        _ => {
            out.extend(ctx.module.split("::").map(ToString::to_string));
        }
    }
    out.extend(segments[index..].iter().cloned());
    Some(out.join("::"))
}
