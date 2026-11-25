//! Compile-time call-site instrumentation for the `telemetry` crate.
//!
//! `#[trace]` wraps the annotated function in a span and every non-noise call
//! expression inside its body in a short-lived guard. With the `enabled`
//! feature off the attribute is a pass-through and the crate compiles to
//! nothing.

#[cfg(feature = "enabled")]
use proc_macro2::{Ident, Span, TokenStream};
#[cfg(feature = "enabled")]
use quote::quote;
#[cfg(feature = "enabled")]
use std::collections::BTreeSet;
#[cfg(feature = "enabled")]
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    visit_mut::{self, VisitMut},
    Error, Expr, ItemFn, LitStr, Result, Token,
};

#[proc_macro_attribute]
pub fn trace(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    #[cfg(feature = "enabled")]
    {
        expand(attr.into(), item.into()).into()
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = attr;
        item
    }
}

#[cfg(feature = "enabled")]
fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut function: ItemFn = match syn::parse2(item.clone()) {
        Ok(function) => function,
        Err(_) => return item,
    };

    let args: TraceArgs = match syn::parse2(attr) {
        Ok(args) => args,
        Err(error) => return error.to_compile_error(),
    };

    if function.sig.constness.is_some() {
        return quote!(#function);
    }

    let name = args
        .name
        .unwrap_or_else(|| LitStr::new(&ident_name(&function.sig.ident), Span::call_site()));

    let mut body = (*function.block).clone();
    if !args.span_only {
        let mut instrumenter = Instrumenter { skip: args.skip };
        instrumenter.visit_block_mut(&mut body);
    }

    let stmts = body.stmts;
    let guard = Ident::new("__telemetry_span", Span::mixed_site());
    *function.block = parse_quote!({
        let #guard = ::telemetry::SpanGuard::enter_function(#name);
        #(#stmts)*
    });

    quote!(#function)
}

#[cfg(feature = "enabled")]
#[derive(Default)]
struct TraceArgs {
    name: Option<LitStr>,
    skip: BTreeSet<String>,
    span_only: bool,
}

#[cfg(feature = "enabled")]
impl Parse for TraceArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut args = TraceArgs::default();
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            match ident.to_string().as_str() {
                "name" => {
                    input.parse::<Token![=]>()?;
                    args.name = Some(input.parse()?);
                }
                "skip" => {
                    input.parse::<Token![=]>()?;
                    let content;
                    bracketed!(content in input);
                    let names = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
                    args.skip.extend(names.iter().map(LitStr::value));
                }
                "span_only" => args.span_only = true,
                other => {
                    return Err(Error::new(
                        ident.span(),
                        format!(
                            "unknown telemetry argument `{other}`; expected `name`, `skip`, or `span_only`"
                        ),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(args)
    }
}

#[cfg(feature = "enabled")]
struct Instrumenter {
    skip: BTreeSet<String>,
}

#[cfg(feature = "enabled")]
impl Instrumenter {
    fn should_wrap(&self, name: &str) -> bool {
        !name.is_empty()
            && !is_noise(name)
            && !self.skip.contains(name)
            && !name.starts_with("__telemetry")
    }
}

#[cfg(feature = "enabled")]
impl VisitMut for Instrumenter {
    fn visit_item_mut(&mut self, _item: &mut syn::Item) {}

    fn visit_expr_repeat_mut(&mut self, repeat: &mut syn::ExprRepeat) {
        self.visit_expr_mut(&mut repeat.expr);
    }

    fn visit_generic_argument_mut(&mut self, argument: &mut syn::GenericArgument) {
        if !matches!(argument, syn::GenericArgument::Const(_)) {
            visit_mut::visit_generic_argument_mut(self, argument);
        }
    }

    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Const(_) => {}
            Expr::Await(await_expr) => {
                let name = callee_name(&await_expr.base);
                match await_expr.base.as_mut() {
                    Expr::Call(call) => {
                        self.visit_expr_mut(&mut call.func);
                        for arg in &mut call.args {
                            self.visit_expr_mut(arg);
                        }
                    }
                    Expr::MethodCall(method) => {
                        self.visit_expr_mut(&mut method.receiver);
                        for arg in &mut method.args {
                            self.visit_expr_mut(arg);
                        }
                    }
                    base => self.visit_expr_mut(base),
                }
                if let Some(name) = name.filter(|name| self.should_wrap(name)) {
                    wrap_await(expr, &name);
                }
            }
            Expr::Call(_) | Expr::MethodCall(_) => {
                let name = match expr {
                    Expr::Call(call) if !is_std_path(&call.func) => callee_name(&call.func),
                    Expr::Call(_) => None,
                    Expr::MethodCall(method) => Some(ident_name(&method.method)),
                    _ => None,
                };
                visit_mut::visit_expr_mut(self, expr);
                if let Some(name) = name.filter(|name| self.should_wrap(name)) {
                    wrap(expr, &name);
                }
            }
            other => visit_mut::visit_expr_mut(self, other),
        }
    }
}

#[cfg(feature = "enabled")]
fn wrap(expr: &mut Expr, name: &str) {
    let name = LitStr::new(name, Span::call_site());
    let original = std::mem::replace(expr, Expr::Verbatim(TokenStream::new()));
    *expr = parse_quote!(::telemetry::finish_call(
        ::telemetry::SpanGuard::enter(#name),
        #original
    ));
}

#[cfg(feature = "enabled")]
fn wrap_await(expr: &mut Expr, name: &str) {
    let Expr::Await(await_expr) = expr else {
        return;
    };
    let name = LitStr::new(name, Span::call_site());
    let base = std::mem::replace(
        &mut await_expr.base,
        Box::new(Expr::Verbatim(TokenStream::new())),
    );
    *expr = parse_quote!(::telemetry::finish_await(
        ::telemetry::SpanGuard::enter(#name),
        #base
    )
    .await);
}

#[cfg(feature = "enabled")]
fn callee_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Call(call) => callee_name(&call.func),
        Expr::MethodCall(method) => Some(ident_name(&method.method)),
        Expr::Paren(paren) => callee_name(&paren.expr),
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| ident_name(&segment.ident)),
        _ => None,
    }
}

#[cfg(feature = "enabled")]
fn ident_name(ident: &Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_string()
}

#[cfg(feature = "enabled")]
fn is_std_path(expr: &Expr) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    let Some(first) = path.path.segments.first() else {
        return false;
    };
    matches!(first.ident.to_string().as_str(), "std" | "core" | "alloc")
}

#[cfg(feature = "enabled")]
fn is_noise(name: &str) -> bool {
    matches!(
        name,
        "clone"
            | "clone_from"
            | "unwrap"
            | "unwrap_or"
            | "unwrap_or_else"
            | "unwrap_or_default"
            | "expect"
            | "expect_err"
            | "ok"
            | "err"
            | "is_ok"
            | "is_err"
            | "is_some"
            | "is_none"
            | "as_ref"
            | "as_mut"
            | "as_str"
            | "as_slice"
            | "as_bytes"
            | "as_path"
            | "as_deref"
            | "to_string"
            | "to_owned"
            | "to_vec"
            | "to_path_buf"
            | "into"
            | "from"
            | "from_millis"
            | "from_micros"
            | "from_nanos"
            | "from_secs"
            | "from_secs_f32"
            | "from_secs_f64"
            | "new"
            | "default"
            | "wrapping_add"
            | "wrapping_sub"
            | "wrapping_mul"
            | "wrapping_div"
            | "wrapping_rem"
            | "wrapping_neg"
            | "wrapping_shl"
            | "wrapping_shr"
            | "saturating_add"
            | "saturating_sub"
            | "saturating_mul"
            | "saturating_div"
            | "checked_add"
            | "checked_sub"
            | "checked_mul"
            | "checked_div"
            | "checked_rem"
            | "overflowing_add"
            | "overflowing_sub"
            | "overflowing_mul"
            | "rotate_left"
            | "rotate_right"
            | "leading_zeros"
            | "trailing_zeros"
            | "count_ones"
            | "abs"
            | "min"
            | "max"
            | "pow"
            | "as_nanos"
            | "as_micros"
            | "as_millis"
            | "as_secs"
            | "subsec_nanos"
            | "subsec_micros"
            | "subsec_millis"
            | "to_le_bytes"
            | "to_be_bytes"
            | "to_bits"
            | "from_bits"
            | "now"
            | "len"
            | "is_empty"
            | "iter"
            | "iter_mut"
            | "into_iter"
            | "collect"
            | "map"
            | "and_then"
            | "map_err"
            | "ok_or"
            | "ok_or_else"
            | "or"
            | "or_else"
            | "insert"
            | "get"
            | "get_mut"
            | "contains"
            | "contains_key"
            | "push"
            | "pop"
            | "first"
            | "last"
            | "next"
            | "extend"
            | "borrow"
            | "borrow_mut"
            | "deref"
            | "drop"
            | "record_attribute"
            | "record_error"
            | "record_error_detail"
            | "record_error_message"
            | "Some"
            | "Ok"
            | "Err"
    )
}

#[cfg(all(test, feature = "enabled"))]
mod tests {
    use super::*;

    #[test]
    fn filters_noise_by_name() {
        assert!(is_noise("clone"));
        assert!(is_noise("unwrap"));
        assert!(is_noise("wrapping_add"));
        assert!(is_noise("saturating_mul"));
        assert!(!is_noise("fetch_staged"));
        assert!(!is_noise("compute_diff"));
    }

    #[test]
    fn rejects_unknown_arguments() {
        let parsed = syn::parse2::<TraceArgs>(quote!(nope = "x"));
        assert!(parsed.is_err());
    }

    #[test]
    fn adds_a_span_and_call_guards() {
        let output = expand(
            TokenStream::new(),
            quote!(
                fn load() {
                    let fetched = fetch().await;
                    let copy = fetched.clone();
                    consume(copy);
                }
            ),
        )
        .to_string();

        assert!(output.contains("SpanGuard"));
        assert!(output.contains("\"load\""));
        assert!(output.contains("\"fetch\""));
        assert!(output.contains("\"consume\""));
        assert!(!output.contains("\"clone\""));
    }

    #[test]
    fn renames_the_root_span() {
        let output = expand(
            quote!(name = "diff.stream"),
            quote!(
                fn load() {
                    fetch();
                }
            ),
        )
        .to_string();

        assert!(output.contains("\"diff.stream\""));
        assert!(!output.contains("\"load\""));
    }

    #[test]
    fn leaves_const_functions_alone() {
        let input = quote!(
            const fn load() -> usize {
                fetch()
            }
        );
        assert!(!expand(TokenStream::new(), input)
            .to_string()
            .contains("SpanGuard"));
    }

    #[test]
    fn skips_standard_library_calls() {
        let output = expand(
            TokenStream::new(),
            quote!(
                fn load() {
                    std::fs::read("x");
                }
            ),
        )
        .to_string();

        assert!(!output.contains("\"read\""));
    }

    #[test]
    fn honours_skip_and_span_only() {
        let skipped = expand(
            quote!(skip = ["expensive_but_known"]),
            quote!(
                fn load() {
                    expensive_but_known();
                }
            ),
        )
        .to_string();
        assert!(!skipped.contains("\"expensive_but_known\""));

        let span_only = expand(
            quote!(span_only),
            quote!(
                fn load() {
                    expensive_but_known();
                }
            ),
        )
        .to_string();
        assert!(!span_only.contains("\"expensive_but_known\""));
    }
}
