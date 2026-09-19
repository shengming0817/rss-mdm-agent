//! Regenerate wire DTOs from the product schema. No runtime schema ownership.
use quote::quote;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("schema path required")?;
    let schema =
        serde_json::from_slice::<schemars_codegen::schema::RootSchema>(&std::fs::read(path)?)?;
    let mut types = typify::TypeSpace::default();
    types.add_root_schema(schema)?;
    let mut file: syn::File = syn::parse2(types.to_stream())?;
    let mut debug = Vec::new();
    for item in &mut file.items {
        // typify does not emit documentation for enum variants. Their parent schema
        // owns semantics; document each generated serialized alternative here.
        if let syn::Item::Enum(enumeration) = item {
            for variant in &mut enumeration.variants {
                // Inline object schemas become named types; typify moves their
                // description to that type and leaves the containing field bare.
                for field in &mut variant.fields {
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
            syn::Item::Struct(s) => (&s.ident, &mut s.attrs),
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
