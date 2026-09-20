//! Regenerate wire DTOs from the product schema. No runtime schema ownership.
use quote::quote;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("schema path required")?;
    let mut schema: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    // Event tags are nested in body. Derive Rust variant names from those exact
    // schema discriminators rather than unstable oneOf array positions.
    if let Some(events) = schema["$defs"]["Event"]["oneOf"].as_array_mut() {
        let mut names = std::collections::BTreeSet::new();
        for event in events {
            let properties = &event["properties"]["body"]["properties"];
            let mut parts: Vec<String> = ["type", "state", "status", "operation"]
                .iter()
                .filter_map(|key| {
                    let property = &properties[key];
                    property["const"].as_str().map(str::to_owned).or_else(|| {
                        property["enum"].as_array().map(|values| {
                            values
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .collect::<Vec<_>>()
                                .join("_")
                        })
                    })
                })
                .collect();
            if let Some(tag) = properties["acknowledgement"]["properties"]["type"]["const"].as_str()
            {
                parts.push(tag.to_owned());
            }
            let name = format!("Event_{}", parts.join("_"));
            if !names.insert(name.clone()) {
                return Err("event discriminators must generate unique Rust variant names".into());
            }
            event["title"] = serde_json::Value::String(name);
        }
    }
    project_constants(&mut schema);
    let schema = serde_json::from_value::<schemars_codegen::schema::RootSchema>(schema)?;
    let mut types = typify::TypeSpace::default();
    types.add_root_schema(schema)?;
    let mut file: syn::File = syn::parse2(types.to_stream())?;
    box_command_record(&mut file)?;
    let mut debug = Vec::new();
    for item in &mut file.items {
        // typify does not emit documentation for enum variants. Their parent schema
        // owns semantics; document each generated serialized alternative here.
        if let syn::Item::Enum(enumeration) = item {
            for variant in &mut enumeration.variants {
                // Only named fields: typify's tuple variants also have generated From impls.
                if let syn::Fields::Named(fields) = &mut variant.fields {
                    for field in &mut fields.named {
                        if matches!(&field.ty, syn::Type::Path(p)
                            if p.path.is_ident("SurfaceState") || p.path.is_ident("Event") || p.path.is_ident("EventSurfaceBody"))
                        {
                            let ty = &field.ty;
                            field.ty = syn::parse_quote!(::std::boxed::Box<#ty>);
                        }
                    }
                }
                // Inline object schemas become named types; typify moves their
                // description to that type and leaves the containing field bare.
                document_fields(&mut variant.fields);
                if !variant.attrs.iter().any(|a| a.path().is_ident("doc")) {
                    let doc = format!(
                        "`{}` alternative; see the parent type's schema contract.",
                        variant.ident
                    );
                    variant.attrs.push(syn::parse_quote!(#[doc = #doc]));
                }
            }
        }
        let (name, attrs) = match item {
            syn::Item::Struct(s) => {
                document_fields(&mut s.fields);
                (&s.ident, &mut s.attrs)
            }
            syn::Item::Enum(e) => (&e.ident, &mut e.attrs),
            _ => continue,
        };
        for attr in attrs.iter_mut().filter(|a| a.path().is_ident("derive")) {
            let paths = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )?;
            let paths: Vec<_> = paths
                .into_iter()
                .filter(|p| p.segments.last().is_none_or(|s| s.ident != "Debug"))
                .collect();
            *attr = syn::parse_quote!(#[derive(#(#paths),*)]);
        }
        debug.push(quote! { impl std::fmt::Debug for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!(#name), "([redacted])"))
            }
        }});
    }
    println!(
        "// @generated from packages/ai-contract/schema/runtime.schema.json. Do not edit.\n{}",
        quote! { #file #(#debug)* }
    );
    Ok(())
}

// typify 0.8 deliberately discards const_value (convert.rs). A singleton enum
// has identical JSON Schema semantics and preserves the discriminator in Rust.
fn project_constants(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(constant) = fields.remove("const") {
                fields.insert("enum".into(), serde_json::Value::Array(vec![constant]));
            }
            for child in fields.values_mut() {
                project_constants(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                project_constants(child);
            }
        }
        _ => (),
    }
}

fn document_fields(fields: &mut syn::Fields) {
    for field in fields {
        if !field.attrs.iter().any(|a| a.path().is_ident("doc")) {
            let doc = format!(
                "`{}` member; see its generated type and parent schema.",
                field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            );
            field.attrs.push(syn::parse_quote!(#[doc = #doc]));
        }
    }
}

// This representation optimization must rewrite the variant and its constructor together.
fn box_command_record(file: &mut syn::File) -> Result<(), &'static str> {
    let (mut variants, mut conversions) = (0, 0);
    for item in &mut file.items {
        if let syn::Item::Enum(enumeration) = item {
            if enumeration.ident == "WireRecord" {
                for variant in &mut enumeration.variants {
                    if variant.ident != "CommandRecord" {
                        continue;
                    }
                    let syn::Fields::Unnamed(fields) = &mut variant.fields else {
                        return Err("CommandRecord variant shape changed");
                    };
                    if fields.unnamed.len() != 1 {
                        return Err("CommandRecord tuple arity changed");
                    }
                    let field = fields
                        .unnamed
                        .first_mut()
                        .ok_or("CommandRecord field missing")?;
                    if !matches!(&field.ty, syn::Type::Path(path) if path.path.is_ident("CommandRecord"))
                    {
                        return Err("CommandRecord type changed");
                    }
                    let ty = &field.ty;
                    field.ty = syn::parse_quote!(::std::boxed::Box<#ty>);
                    variants += 1;
                }
            }
        }
        if let syn::Item::Impl(implementation) = item {
            if matches!(&*implementation.self_ty, syn::Type::Path(path) if path.path.is_ident("WireRecord"))
            {
                for item in &mut implementation.items {
                    if let syn::ImplItem::Fn(function) = item {
                        if function.sig.ident != "from" {
                            continue;
                        }
                        if let Some(syn::Stmt::Expr(syn::Expr::Call(call), _)) =
                            function.block.stmts.last_mut()
                        {
                            if matches!(&*call.func, syn::Expr::Path(path) if path.path.segments.last().is_some_and(|part| part.ident == "CommandRecord"))
                            {
                                if call.args.len() != 1 {
                                    return Err("CommandRecord conversion arity changed");
                                }
                                let value = call
                                    .args
                                    .first_mut()
                                    .ok_or("CommandRecord conversion missing")?;
                                *value = syn::parse_quote!(::std::boxed::Box::new(#value));
                                conversions += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    if (variants, conversions) != (1, 1) {
        return Err("CommandRecord boxing must match exactly one variant and one conversion");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const VARIANT: &str = "enum WireRecord { CommandRecord(CommandRecord) }";
    const CONVERSION: &str = "impl From<CommandRecord> for WireRecord { fn from(value: CommandRecord) -> Self { Self::CommandRecord(value) } }";
    #[test]
    fn boxing_requires_exactly_one_variant_and_conversion() {
        let mut valid = syn::parse_file(&format!("{VARIANT} {CONVERSION}")).unwrap();
        box_command_record(&mut valid).unwrap();
        assert_eq!(quote!(#valid).to_string().matches("Box").count(), 2);
        for source in [VARIANT.to_string(), CONVERSION.to_string(), format!("{VARIANT} {CONVERSION} {CONVERSION}"), format!("{VARIANT} impl From<CommandRecord> for WireRecord {{ fn from(value: CommandRecord) -> Self {{ return Self::CommandRecord(value); }} }}")] {
            assert!(box_command_record(&mut syn::parse_file(&source).unwrap()).is_err());
        }
    }
}
