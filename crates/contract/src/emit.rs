use std::collections::BTreeSet;

use syn::ext::IdentExt;
use syn::{GenericArgument, PathArguments, Type};

use crate::model::{rename, Command, Fields, ItemKind, TypeItem, Variant, Workspace};

const EXTRA_MODULE_ROOTS: &[&str] = &["forge_core::dto"];

const EXTRA_TYPE_ROOTS: &[&str] = &[
    "git::error::SerializedError",
    "git::runtime::invalidation::RepositoryInvalidation",
    "gitau_lib::state::SessionRepository",
];

const RESULT_OVERRIDES: &[(&str, &str)] = &[("session_load", "SessionDocument | null")];

const ARG_OVERRIDES: &[(&str, &str, &str)] = &[("session_save", "doc", "SessionDocument")];

const MANUAL_TS_TYPES: &[(&str, &str)] = &[
    ("ObjectId", "string"),
    ("RevisionSpec", "string"),
    ("SharedRows", "DiffRow[]"),
];

const TYPE_RENAMES: &[(&str, &str)] = &[("Style", "SyntaxStyle"), ("TextStyle", "SyntaxStyle")];

pub fn emit(ws: &Workspace) -> Result<(String, String), String> {
    let reachable = collect_reachable(ws)?;
    let input_only = reachable
        .input
        .difference(&reachable.output)
        .cloned()
        .collect();
    let mut emitter = Emitter {
        ws,
        uses_channel: false,
        input_only,
    };
    let protocol = emitter.protocol(&reachable.all)?;
    let commands = emitter.commands(&reachable.all)?;
    Ok((protocol, commands))
}

struct Reachable {
    all: BTreeSet<String>,
    input: BTreeSet<String>,
    output: BTreeSet<String>,
}

fn collect_reachable(ws: &Workspace) -> Result<Reachable, String> {
    let mut input = BTreeSet::new();
    let mut output = BTreeSet::new();

    for command in &ws.commands {
        for arg in &command.args {
            collect_type(ws, &arg.ty, &command.module, &mut input)?;
        }
        collect_type(ws, &command.result, &command.module, &mut output)?;
    }

    let mut extra = BTreeSet::new();
    for item in ws.items.values() {
        if EXTRA_MODULE_ROOTS.contains(&item.module.as_str()) {
            visit_named(ws, &item.fqname, &mut extra)?;
        }
    }

    for root in EXTRA_TYPE_ROOTS {
        if ws.resolve(root).is_some() || ws.aliases.contains_key(*root) {
            visit_named(ws, root, &mut extra)?;
        }
    }

    let mut all = input.clone();
    all.extend(output.iter().cloned());
    all.extend(extra);
    Ok(Reachable { all, input, output })
}

fn collect_type(
    ws: &Workspace,
    ty: &Type,
    module: &str,
    out: &mut BTreeSet<String>,
) -> Result<(), String> {
    match ty {
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return Ok(());
            };
            let ident = segment.ident.unraw().to_string();
            if let Some(inner) = wrapper_inner(&ident, segment) {
                for inner in inner {
                    collect_type(ws, inner, module, out)?;
                }
                return Ok(());
            }
            if is_primitive(&ident) {
                return Ok(());
            }
            let segments = path_segments(&path.path);
            let Some(fqname) = ws.resolve_path(module, &segments) else {
                return Err(format!(
                    "cannot resolve type `{}` in `{module}`",
                    segments.join("::")
                ));
            };
            visit_named(ws, &fqname, out)
        }
        Type::Reference(reference) => collect_type(ws, &reference.elem, module, out),
        Type::Tuple(tuple) => {
            for elem in &tuple.elems {
                collect_type(ws, elem, module, out)?;
            }
            Ok(())
        }
        Type::Slice(slice) => collect_type(ws, &slice.elem, module, out),
        Type::Array(array) => collect_type(ws, &array.elem, module, out),
        Type::Paren(paren) => collect_type(ws, &paren.elem, module, out),
        Type::Group(group) => collect_type(ws, &group.elem, module, out),
        Type::Ptr(ptr) => collect_type(ws, &ptr.elem, module, out),
        Type::Never(_) | Type::Infer(_) => Ok(()),
        _ => Ok(()),
    }
}

fn visit_named(ws: &Workspace, fqname: &str, out: &mut BTreeSet<String>) -> Result<(), String> {
    let Some(item) = ws.resolve(fqname) else {
        return Ok(());
    };
    if !out.insert(item.fqname.clone()) {
        return Ok(());
    }
    if item.manual_serialize {
        return Ok(());
    }
    match &item.kind {
        ItemKind::Alias(ty) => collect_type(ws, ty, &item.module, out),
        ItemKind::Struct(fields) => {
            for field in fields_iter(fields) {
                collect_type(ws, &field.ty, &item.module, out)?;
            }
            Ok(())
        }
        ItemKind::Enum(variants) => {
            for variant in variants {
                for field in fields_iter(&variant.fields) {
                    collect_type(ws, &field.ty, &item.module, out)?;
                }
            }
            Ok(())
        }
    }
}

struct Emitter<'a> {
    ws: &'a Workspace,
    uses_channel: bool,
    input_only: BTreeSet<String>,
}

impl Emitter<'_> {
    fn protocol(&mut self, reachable: &BTreeSet<String>) -> Result<String, String> {
        let mut items = reachable
            .iter()
            .filter_map(|fqname| self.ws.resolve(fqname))
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.name.cmp(&right.name));

        let mut body = String::new();
        for item in &items {
            self.emit_type(&mut body, item)?;
            body.push('\n');
        }
        self.emit_command_maps(&mut body)?;

        let mut out = String::new();
        out.push_str("/**\n");
        out.push_str(" * GENERATED FILE - do not edit by hand.\n");
        out.push_str(" *\n");
        out.push_str(
            " * Source of truth: Rust serde attributes and #[tauri::command] signatures.\n",
        );
        out.push_str(" * Regenerate with: cargo run -p contract --bin export_contract\n");
        out.push_str(" * Freshness is verified by cargo test (contract::tests::generated_contracts_are_fresh).\n");
        out.push_str(" */\n\n");
        if self.uses_channel {
            out.push_str("import type { Channel } from \"@tauri-apps/api/core\";\n");
        }
        out.push_str("import type { SessionDocument, SyntaxStyle } from \"./protocol.manual\";\n");
        out.push_str("\nexport * from \"./protocol.manual\";\n\n");
        out.push_str(&body);
        Ok(out)
    }

    fn emit_type(&mut self, out: &mut String, item: &TypeItem) -> Result<(), String> {
        if TYPE_RENAMES.iter().any(|(from, _)| *from == item.name) {
            return Ok(());
        }
        emit_docs(out, "", &item.docs);
        let name = &item.name;
        if item.manual_serialize {
            let Some(ts) = MANUAL_TS_TYPES
                .iter()
                .find(|(manual, _)| *manual == name)
                .map(|(_, ts)| *ts)
            else {
                return Err(format!(
                    "type `{name}` has a manual Serialize impl; add it to MANUAL_TS_TYPES"
                ));
            };
            out.push_str(&format!("export type {name} = {ts};\n"));
            return Ok(());
        }

        match &item.kind {
            ItemKind::Alias(ty) => {
                let rendered = self.render_type(ty, &item.module)?;
                out.push_str(&format!("export type {name} = {rendered};\n"));
            }
            ItemKind::Struct(fields) => {
                self.emit_struct(out, item, fields)?;
            }
            ItemKind::Enum(variants) => {
                self.emit_enum(out, item, variants)?;
            }
        }
        Ok(())
    }

    fn emit_struct(
        &mut self,
        out: &mut String,
        item: &TypeItem,
        fields: &Fields,
    ) -> Result<(), String> {
        let name = &item.name;
        if item.serde.transparent {
            let field_list = fields_iter(fields);
            let Some(field) = field_list.first() else {
                return Err(format!("transparent struct `{name}` has no fields"));
            };
            let rendered = self.render_type(&field.ty, &item.module)?;
            out.push_str(&format!("export type {name} = {rendered};\n"));
            return Ok(());
        }
        if let Fields::Unnamed(unnamed) = fields {
            if unnamed.len() == 1 {
                let rendered = self.render_type(&unnamed[0].ty, &item.module)?;
                out.push_str(&format!("export type {name} = {rendered};\n"));
                return Ok(());
            }
        }

        let body = self.render_object_body(
            fields,
            item.serde.rename_all.as_deref(),
            &item.module,
            "    ",
            item.serde.default && self.input_only.contains(&item.fqname),
        )?;
        if body.flattened.is_empty() {
            out.push_str(&format!("export interface {name} {{\n"));
            for line in &body.lines {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str("}\n");
            return Ok(());
        }

        let mut parts = body.flattened.clone();
        if !body.lines.is_empty() {
            let mut object = String::from("{\n");
            for line in &body.lines {
                object.push_str(line);
                object.push('\n');
            }
            object.push('}');
            parts.push(object);
        }
        out.push_str(&format!("export type {name} = {};\n", parts.join(" & ")));
        Ok(())
    }

    fn emit_enum(
        &mut self,
        out: &mut String,
        item: &TypeItem,
        variants: &[Variant],
    ) -> Result<(), String> {
        let name = &item.name;
        if item.serde.transparent {
            let Some(variant) = variants.first() else {
                return Err(format!("transparent enum `{name}` has no variants"));
            };
            let Some(payload) = self.render_payload(variant, &item.module, None)? else {
                return Err(format!("transparent enum `{name}` has a unit variant"));
            };
            out.push_str(&format!("export type {name} = {payload};\n"));
            return Ok(());
        }

        if item.serde.untagged {
            let mut parts = Vec::new();
            for variant in variants {
                let payload = self.render_payload(variant, &item.module, None)?;
                parts.push(payload.unwrap_or_else(|| "null".to_owned()));
            }
            out.push_str(&format!("export type {name} =\n"));
            emit_union(out, &parts, "    ");
            return Ok(());
        }

        if let Some(tag) = &item.serde.tag {
            if let Some(content) = &item.serde.content {
                let mut parts = Vec::new();
                for variant in variants {
                    let variant_name = variant_wire_name(variant, item.serde.rename_all.as_deref());
                    let payload = self.render_payload(
                        variant,
                        &item.module,
                        item.serde.rename_all_fields.as_deref(),
                    )?;
                    match payload {
                        Some(payload) => parts.push(format!(
                            "{{ {tag}: \"{variant_name}\"; {content}: {payload} }}"
                        )),
                        None => parts.push(format!("{{ {tag}: \"{variant_name}\" }}")),
                    }
                }
                out.push_str(&format!("export type {name} =\n"));
                emit_union(out, &parts, "    ");
                return Ok(());
            }

            let mut parts = Vec::new();
            for variant in variants {
                let variant_name = variant_wire_name(variant, item.serde.rename_all.as_deref());
                parts.push(self.render_internally_tagged(
                    tag,
                    &variant_name,
                    variant,
                    item,
                    "    ",
                )?);
            }
            out.push_str(&format!("export type {name} =\n"));
            emit_union(out, &parts, "    ");
            return Ok(());
        }

        if variants
            .iter()
            .all(|variant| matches!(variant.fields, Fields::Unit))
        {
            let parts = variants
                .iter()
                .map(|variant| {
                    format!(
                        "\"{}\"",
                        variant_wire_name(variant, item.serde.rename_all.as_deref())
                    )
                })
                .collect::<Vec<_>>();
            out.push_str(&format!("export type {name} =\n"));
            emit_union(out, &parts, "    ");
            return Ok(());
        }

        let mut parts = Vec::new();
        for variant in variants {
            let variant_name = variant_wire_name(variant, item.serde.rename_all.as_deref());
            let payload = self.render_payload(
                variant,
                &item.module,
                item.serde.rename_all_fields.as_deref(),
            )?;
            match payload {
                None => parts.push(format!("\"{variant_name}\"")),
                Some(payload) => {
                    parts.push(format!("{{ {variant_name}: {payload} }}"));
                }
            }
        }
        out.push_str(&format!("export type {name} =\n"));
        emit_union(out, &parts, "    ");
        Ok(())
    }

    fn render_internally_tagged(
        &mut self,
        tag: &str,
        variant_name: &str,
        variant: &Variant,
        item: &TypeItem,
        indent: &str,
    ) -> Result<String, String> {
        match &variant.fields {
            Fields::Unit => Ok(format!("{{ {tag}: \"{variant_name}\" }}")),
            Fields::Unnamed(unnamed) if unnamed.len() == 1 => {
                let inner = self.render_type(&unnamed[0].ty, &item.module)?;
                Ok(format!("{{ {tag}: \"{variant_name}\" }} & {inner}"))
            }
            Fields::Unnamed(_) => Err(format!(
                "internally tagged enum `{}` has a tuple variant",
                item.name
            )),
            Fields::Named(_) => {
                let body = self.render_object_body(
                    &variant.fields,
                    item.serde.rename_all_fields.as_deref(),
                    &item.module,
                    indent,
                    false,
                )?;
                let mut object = format!("{{ {tag}: \"{variant_name}\";");
                let mut extra = body.flattened.clone();
                if !body.lines.is_empty() {
                    let mut fields = String::new();
                    for line in &body.lines {
                        fields.push_str(line.trim_start());
                        fields.push('\n');
                    }
                    object.push('\n');
                    object.push_str(&fields);
                    object.push_str(indent);
                }
                object.push('}');
                if extra.is_empty() {
                    Ok(object)
                } else {
                    extra.insert(0, object);
                    Ok(extra.join(" & "))
                }
            }
        }
    }

    fn render_payload(
        &mut self,
        variant: &Variant,
        module: &str,
        rename_all: Option<&str>,
    ) -> Result<Option<String>, String> {
        match &variant.fields {
            Fields::Unit => Ok(None),
            Fields::Unnamed(unnamed) if unnamed.len() == 1 => {
                Ok(Some(self.render_type(&unnamed[0].ty, module)?))
            }
            Fields::Unnamed(unnamed) => {
                let parts = unnamed
                    .iter()
                    .map(|field| self.render_type(&field.ty, module))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Some(format!("[{}]", parts.join(", "))))
            }
            Fields::Named(_) => {
                let body =
                    self.render_object_body(&variant.fields, rename_all, module, "    ", false)?;
                Ok(Some(render_object(&body)))
            }
        }
    }

    fn render_object_body(
        &mut self,
        fields: &Fields,
        rename_all: Option<&str>,
        module: &str,
        indent: &str,
        struct_default: bool,
    ) -> Result<ObjectBody, String> {
        let mut lines = Vec::new();
        let mut flattened = Vec::new();
        let Some(fields) = named_fields(fields) else {
            return Ok(ObjectBody { lines, flattened });
        };
        for field in fields {
            if field.serde.skip || field.serde.skip_serializing {
                continue;
            }
            let Some(name) = &field.name else {
                continue;
            };
            if field.serde.flatten {
                let mut ty = self.render_type(&field.ty, module)?;
                if option_inner(&field.ty).is_some() {
                    ty = format!("Partial<{ty}>");
                }
                flattened.push(ty);
                continue;
            }
            let wire_name = field
                .serde
                .rename
                .clone()
                .unwrap_or_else(|| rename(name, rename_all));
            let (ty, optional) = self.render_field_type(field, module, struct_default)?;
            emit_docs_lines(&mut lines, indent, &field.docs);
            lines.push(format!(
                "{indent}{wire_name}{}: {ty};",
                if optional { "?" } else { "" }
            ));
        }
        Ok(ObjectBody { lines, flattened })
    }

    fn render_field_type(
        &mut self,
        field: &crate::model::Field,
        module: &str,
        struct_default: bool,
    ) -> Result<(String, bool), String> {
        if let Some(inner) = option_inner(&field.ty) {
            let rendered = self.render_type(inner, module)?;
            if field.serde.skip_serializing_if.is_some() || struct_default {
                Ok((rendered, true))
            } else {
                Ok((format!("{rendered} | null"), false))
            }
        } else {
            let rendered = self.render_type(&field.ty, module)?;
            Ok((
                rendered,
                field.serde.skip_serializing_if.is_some() || struct_default,
            ))
        }
    }

    fn render_type(&mut self, ty: &Type, module: &str) -> Result<String, String> {
        match ty {
            Type::Path(path) => {
                let Some(segment) = path.path.segments.last() else {
                    return Ok("unknown".to_owned());
                };
                let ident = segment.ident.unraw().to_string();
                if let Some(inner) = wrapper_inner(&ident, segment) {
                    return self.render_wrapper(&ident, &inner, module);
                }
                if let Some(primitive) = primitive_ts(&ident) {
                    return Ok(primitive.to_owned());
                }
                let segments = path_segments(&path.path);
                let Some(fqname) = self.ws.resolve_path(module, &segments) else {
                    return Err(format!(
                        "cannot resolve type `{}` in `{module}`",
                        segments.join("::")
                    ));
                };
                let Some(item) = self.ws.resolve(&fqname) else {
                    return Err(format!("cannot find item `{fqname}`"));
                };
                Ok(ts_name(item).to_owned())
            }
            Type::Reference(reference) => self.render_type(&reference.elem, module),
            Type::Tuple(tuple) if tuple.elems.is_empty() => Ok("void".to_owned()),
            Type::Tuple(tuple) => {
                let parts = tuple
                    .elems
                    .iter()
                    .map(|elem| self.render_type(elem, module))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("[{}]", parts.join(", ")))
            }
            Type::Slice(slice) => Ok(list_type(self.render_type(&slice.elem, module)?)),
            Type::Array(array) => Ok(list_type(self.render_type(&array.elem, module)?)),
            Type::Paren(paren) => self.render_type(&paren.elem, module),
            Type::Group(group) => self.render_type(&group.elem, module),
            Type::Ptr(ptr) => self.render_type(&ptr.elem, module),
            Type::Never(_) => Ok("never".to_owned()),
            Type::Infer(_) => Ok("unknown".to_owned()),
            _ => Ok("unknown".to_owned()),
        }
    }

    fn render_wrapper(
        &mut self,
        ident: &str,
        inner: &[&Type],
        module: &str,
    ) -> Result<String, String> {
        match ident {
            "Option" => {
                let inner = inner
                    .first()
                    .ok_or_else(|| "Option without a type argument".to_owned())?;
                Ok(format!("{} | null", self.render_type(inner, module)?))
            }
            "Vec" | "HashSet" | "BTreeSet" => {
                let inner = inner
                    .first()
                    .ok_or_else(|| format!("{ident} without a type argument"))?;
                Ok(list_type(self.render_type(inner, module)?))
            }
            "HashMap" | "BTreeMap" | "IndexMap" => {
                let value = inner
                    .last()
                    .ok_or_else(|| format!("{ident} without a value type"))?;
                Ok(format!(
                    "Record<string, {}>",
                    self.render_type(value, module)?
                ))
            }
            "Box" | "Arc" | "Rc" | "Cell" | "RefCell" | "Mutex" => {
                let inner = inner
                    .first()
                    .ok_or_else(|| format!("{ident} without a type argument"))?;
                self.render_type(inner, module)
            }
            "Cow" => {
                let inner = inner
                    .last()
                    .ok_or_else(|| "Cow without a type argument".to_owned())?;
                if let Type::Path(path) = inner {
                    if path.path.is_ident("str") {
                        return Ok("string".to_owned());
                    }
                }
                self.render_type(inner, module)
            }
            "Channel" => {
                self.uses_channel = true;
                let inner = inner
                    .first()
                    .ok_or_else(|| "Channel without a payload type".to_owned())?;
                Ok(format!("Channel<{}>", self.render_type(inner, module)?))
            }
            _ => {
                let inner = inner
                    .last()
                    .ok_or_else(|| format!("{ident} without a type argument"))?;
                self.render_type(inner, module)
            }
        }
    }

    fn emit_command_maps(&mut self, out: &mut String) -> Result<(), String> {
        out.push_str("export type CommandName =\n");
        let names = self
            .ws
            .commands
            .iter()
            .map(|command| format!("    | \"{}\"", command.name))
            .collect::<Vec<_>>();
        out.push_str(&names.join("\n"));
        out.push_str(";\n\n");

        out.push_str("export interface CommandArgs {\n");
        for command in &self.ws.commands {
            let args = self.command_args(command)?;
            out.push_str(&format!("    {}: {args};\n", command.name));
        }
        out.push_str("}\n\n");

        out.push_str("export interface CommandResults {\n");
        for command in &self.ws.commands {
            let result = self.command_result(command)?;
            out.push_str(&format!("    {}: {result};\n", command.name));
        }
        out.push_str("}\n");
        Ok(())
    }

    fn command_args(&mut self, command: &Command) -> Result<String, String> {
        if command.args.is_empty() {
            return Ok("{}".to_owned());
        }
        let mut fields = Vec::new();
        for arg in &command.args {
            let override_ty = ARG_OVERRIDES
                .iter()
                .find(|(command_name, arg_name, _)| {
                    *command_name == command.name && *arg_name == arg.name
                })
                .map(|(_, _, ty)| *ty);
            let ty = match override_ty {
                Some(ty) => ty.to_owned(),
                None => {
                    if let Some(inner) = option_inner(&arg.ty) {
                        self.render_type(inner, &command.module)?
                    } else {
                        self.render_type(&arg.ty, &command.module)?
                    }
                }
            };
            fields.push(format!(
                "{}{}: {ty}",
                arg.name,
                if arg.optional { "?" } else { "" }
            ));
        }
        Ok(format!("{{ {} }}", fields.join("; ")))
    }

    fn command_result(&mut self, command: &Command) -> Result<String, String> {
        if let Some((_, ty)) = RESULT_OVERRIDES
            .iter()
            .find(|(name, _)| *name == command.name)
        {
            return Ok((*ty).to_owned());
        }
        match &command.result {
            Type::Tuple(tuple) if tuple.elems.is_empty() => Ok("void".to_owned()),
            Type::Path(path) if path.path.is_ident("()") => Ok("void".to_owned()),
            other => self.render_type(other, &command.module),
        }
    }

    fn commands(&mut self, reachable: &BTreeSet<String>) -> Result<String, String> {
        let mut out = String::new();
        out.push_str("/**\n");
        out.push_str(" * GENERATED FILE - do not edit by hand.\n");
        out.push_str(" *\n");
        out.push_str(
            " * Source of truth: #[tauri::command] signatures and the generated protocol.\n",
        );
        out.push_str(" * Regenerate with: cargo run -p contract --bin export_contract\n");
        out.push_str(" * Freshness is verified by cargo test (contract::tests::generated_contracts_are_fresh).\n");
        out.push_str(" */\n\n");
        out.push_str("import type { CommandArgs, CommandResults } from \"../protocol\";\n");
        out.push_str("import { invokeCommand } from \"./invoke\";\n\n");
        out.push_str("export const commands = {\n");
        for command in &self.ws.commands {
            emit_docs(&mut out, "    ", &command.docs);
            if command.args.is_empty() {
                out.push_str(&format!(
                    "    {}: () =>\n        invokeCommand<CommandResults[\"{}\"]>(\"{}\"),\n",
                    command.name, command.name, command.name
                ));
            } else {
                out.push_str(&format!(
                    "    {}: (args: CommandArgs[\"{}\"]) =>\n        invokeCommand<CommandResults[\"{}\"]>(\"{}\", {{ args }}),\n",
                    command.name, command.name, command.name, command.name
                ));
            }
        }
        out.push_str("};\n");
        let _ = reachable;
        Ok(out)
    }
}

struct ObjectBody {
    lines: Vec<String>,
    flattened: Vec<String>,
}

fn render_object(body: &ObjectBody) -> String {
    let mut parts = body.flattened.clone();
    if !body.lines.is_empty() {
        let mut object = String::from("{\n");
        for line in &body.lines {
            object.push_str(line);
            object.push('\n');
        }
        object.push('}');
        parts.push(object);
    }
    if parts.is_empty() {
        "{}".to_owned()
    } else {
        parts.join(" & ")
    }
}

fn emit_union(out: &mut String, parts: &[String], indent: &str) {
    for (index, part) in parts.iter().enumerate() {
        out.push_str(indent);
        if index > 0 {
            out.push_str("| ");
        }
        out.push_str(part);
        out.push('\n');
    }
    out.pop();
    out.push_str(";\n");
}

fn emit_docs_lines(out: &mut Vec<String>, indent: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    let mut rendered = String::new();
    emit_docs(&mut rendered, indent, text);
    for line in rendered.lines() {
        out.push(line.to_owned());
    }
}

fn emit_docs(out: &mut String, indent: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str(indent);
    out.push_str("/**\n");
    for line in text.lines() {
        out.push_str(indent);
        out.push_str(" *");
        if !line.is_empty() {
            out.push(' ');
            out.push_str(&line.replace("*/", "* /"));
        }
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str(" */\n");
}

fn fields_iter(fields: &Fields) -> Vec<&crate::model::Field> {
    match fields {
        Fields::Named(fields) => fields.iter().collect(),
        Fields::Unnamed(fields) => fields.iter().collect(),
        Fields::Unit => Vec::new(),
    }
}

fn named_fields(fields: &Fields) -> Option<&[crate::model::Field]> {
    match fields {
        Fields::Named(fields) => Some(fields),
        _ => None,
    }
}

fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect()
}

fn path_args(segment: &syn::PathSegment) -> Vec<&Type> {
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Vec::new();
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect()
}

fn wrapper_inner<'a>(ident: &str, segment: &'a syn::PathSegment) -> Option<Vec<&'a Type>> {
    if !matches!(
        ident,
        "Option"
            | "Vec"
            | "HashSet"
            | "BTreeSet"
            | "HashMap"
            | "BTreeMap"
            | "IndexMap"
            | "Box"
            | "Arc"
            | "Rc"
            | "Cell"
            | "RefCell"
            | "Mutex"
            | "Cow"
            | "Channel"
    ) {
        return None;
    }
    Some(path_args(segment))
}

fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    path_args(segment).first().copied()
}

fn primitive_ts(ident: &str) -> Option<&'static str> {
    match ident {
        "String" | "str" | "char" | "PathBuf" | "Path" => Some("string"),
        "bool" => Some("boolean"),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128"
        | "isize" | "f32" | "f64" | "NonZeroU8" | "NonZeroU16" | "NonZeroU32" | "NonZeroU64"
        | "NonZeroUsize" | "NonZeroI8" | "NonZeroI16" | "NonZeroI32" | "NonZeroI64"
        | "NonZeroIsize" => Some("number"),
        "Value" => Some("unknown"),
        _ => None,
    }
}

fn is_primitive(ident: &str) -> bool {
    primitive_ts(ident).is_some()
}

fn list_type(inner: String) -> String {
    if inner.contains('|') || inner.contains(" & ") {
        format!("({inner})[]")
    } else {
        format!("{inner}[]")
    }
}

fn variant_wire_name(variant: &Variant, rename_all: Option<&str>) -> String {
    variant
        .serde
        .rename
        .clone()
        .unwrap_or_else(|| rename(&variant.name, rename_all))
}

fn ts_name(item: &TypeItem) -> &str {
    TYPE_RENAMES
        .iter()
        .find(|(from, _)| *from == item.name)
        .map(|(_, to)| *to)
        .unwrap_or(&item.name)
}
