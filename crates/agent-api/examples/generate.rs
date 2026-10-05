//! Schema-to-Rust projection. Runtime decoding retains the untouched authoritative schemas.
use quote::quote;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

fn key(values: impl Iterator<Item = String>) -> String {
    let mut keys: Vec<_> = values.collect();
    keys.sort();
    keys.join("|")
}
fn width(value: &mut Value, kind: &str) {
    if let Some(types) = value.get("type").and_then(Value::as_array) {
        let nullable = types.iter().any(|v| v == "null");
        *value = if nullable {
            json!({"anyOf":[{"type":"integer","format":format!("agent-{kind}")},{"type":"null"}]})
        } else {
            json!({"type":"integer","format":format!("agent-{kind}")})
        };
        return;
    }
    if value["type"] == "integer" {
        value["format"] = json!(format!("agent-{kind}"));
    }
    if let Some(items) = value.get_mut("items") {
        width(items, kind);
    }
    for union in ["oneOf", "anyOf"] {
        if let Some(values) = value.get_mut(union).and_then(Value::as_array_mut) {
            for v in values {
                width(v, kind);
            }
        }
    }
}
fn normalize(
    value: &Value,
    name: &str,
    hints: &Value,
    definitions: &mut BTreeMap<String, Value>,
    root: bool,
) -> Value {
    let Some(mut fields) = value.as_object().cloned() else {
        return value.clone();
    };
    if let Some(reference) = fields.get("$ref").and_then(Value::as_str) {
        let old = reference.trim_start_matches("#/$defs/");
        let mapped = match old {
            "nonNilUuid" => return json!({"type":"string","format":"uuid"}),
            "fieldType" => "ValueType",
            "fieldDefinition" => "FieldDefinition",
            "scalar" => "Scalar",
            "collection" => "CollectionDefinition",
            "fieldValue" => "FieldValue",
            "bindingProfile" => "BindingProfile",
            "grantReceipt" => "GrantReceipt",
            "activationReceipt" => "ActivationReceipt",
            "runtimeProfile" => "RuntimeProfile",
            _ => old,
        };
        return json!({"$ref":format!("#/definitions/{mapped}")});
    }
    if name.ends_with("ParametersValue") || name.ends_with("ParameterSourceValue") {
        return json!({});
    }
    let object = fields.get("properties").and_then(Value::as_object);
    if let Some(props) = object {
        let shape = key(props.keys().cloned());
        let chosen = hints["objectNames"][&shape]
            .as_str()
            .unwrap_or(name)
            .to_owned();
        if !root && !props.contains_key("kind") && !props.contains_key("state") {
            let result = normalize(value, &chosen, hints, definitions, true);
            definitions.entry(chosen.clone()).or_insert(result);
            return json!({"$ref":format!("#/definitions/{chosen}")});
        }
        let projected: serde_json::Map<_, _> = props
            .iter()
            .map(|(field, shape)| {
                let field_name = format!("{chosen}{field}");
                if ["TaskSpec", "SelfServiceInvoke"].contains(&chosen.as_str())
                    && field == "parameters"
                {
                    return (
                        field.clone(),
                        json!({"type":"object","additionalProperties":{}}),
                    );
                }
                if chosen == "ScriptParameterSourceVariant0" && field == "value" {
                    return (field.clone(), json!({}));
                }
                if field == "payload" && chosen == "SignedTask" {
                    return (field.clone(), json!({"$ref":"#/definitions/TaskPayload"}));
                }
                if field == "event" && chosen == "TaskEventRequest" && shape.get("$ref").is_none() {
                    let projected = normalize(shape, "TaskEvent", hints, definitions, true);
                    definitions.insert("TaskEvent".into(), projected);
                    return (field.clone(), json!({"$ref":"#/definitions/TaskEvent"}));
                }
                if field == "body" && chosen == "ReportRequest" && shape.get("$ref").is_none() {
                    let body = shape.clone();
                    let projected = normalize(&body, "ReportBody", hints, definitions, true);
                    definitions.insert("ReportBody".into(), projected);
                    return (field.clone(), json!({"$ref":"#/definitions/ReportBody"}));
                }
                let width = hints["integerWidths"][format!("{chosen}.{field}")].as_str();
                let mut shape = if field == "secret" {
                    json!({"type":"string","format":"agent-secret"})
                } else {
                    normalize(shape, &field_name, hints, definitions, false)
                };
                if let Some(kind) = width {
                    self::width(&mut shape, kind);
                }
                (field.clone(), shape)
            })
            .collect();
        let mut out = json!({"type":"object","properties":projected,"additionalProperties":fields.get("additionalProperties").cloned().unwrap_or(json!(false))});
        if let Some(required) = fields.get("required") {
            out["required"] = required.clone();
        }
        // Schemas use a missing limit to mean an open JSON object.
        if let Some(other) = fields.get("additionalProperties").filter(|s| s.is_object()) {
            out["additionalProperties"] =
                normalize(other, &format!("{chosen}Value"), hints, definitions, false);
        }
        return out;
    }
    if let Some(values) = fields.get("enum").and_then(Value::as_array) {
        if values.iter().all(Value::is_string) {
            let shape = key(values.iter().map(|v| v.as_str().unwrap().to_owned()));
            if let Some(named) = hints["enumNames"][shape]["name"].as_str() {
                if !root {
                    definitions
                        .entry(named.into())
                        .or_insert(json!({"type":"string","enum":values}));
                    return json!({"$ref":format!("#/definitions/{named}")});
                }
            }
            return json!({"type":"string","enum":values});
        }
    }
    if let Some(constant) = fields.get("const") {
        if constant.is_string() {
            return json!({"type":"string","enum":[constant]});
        }
        return if constant.is_boolean() {
            json!({"type":"boolean"})
        } else {
            json!({"type":"integer","format":"agent-u8"})
        };
    }
    if let Some(variants) = fields.get("oneOf").and_then(Value::as_array) {
        for variant in variants {
            if let Some(label) = variant["properties"]["kind"]["const"].as_str() {
                if let Some(payload) = hints["enumPayloads"][name][label].as_str() {
                    let mut body = variant.clone();
                    body["properties"].as_object_mut().unwrap().remove("kind");
                    if let Some(required) = body["required"].as_array_mut() {
                        required.retain(|v| v.as_str() != Some("kind"));
                    }
                    let projected = normalize(&body, payload, hints, definitions, true);
                    definitions.insert(payload.into(), projected);
                }
            }
        }
    }
    for union in ["oneOf", "anyOf"] {
        if let Some(values) = fields.get(union).and_then(Value::as_array) {
            return json!({union:values.iter().enumerate().map(|(i,v)|{
                let mut variant_hints=hints.clone();
                let label=v["properties"]["kind"]["const"].as_str().or_else(||v["properties"]["state"]["const"].as_str());
                if let Some(label)=label {if let Some(properties)=v["properties"].as_object(){for field in properties.keys(){if let Some(kind)=hints["integerWidths"][format!("{name}.{label}.{field}")].as_str(){variant_hints["integerWidths"][format!("{name}Variant{i}.{field}")]=json!(kind);}}}}
                normalize(v,&format!("{name}Variant{i}"),&variant_hints,definitions,false)
            }).collect::<Vec<_>>()});
        }
    }
    match fields.get("type").cloned() {
        Some(Value::String(t)) if t == "string" => {
            if fields.get("format").and_then(Value::as_str) == Some("uuid")
                || fields
                    .get("pattern")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.contains("{8}-") && s.contains("{12}"))
            {
                json!({"type":"string","format":"uuid"})
            } else {
                json!({"type":"string"})
            }
        }
        Some(Value::String(t)) if t == "integer" => {
            let width = if fields
                .get("minimum")
                .and_then(Value::as_i64)
                .is_some_and(|v| v >= 0)
            {
                match fields.get("maximum").and_then(Value::as_u64) {
                    Some(255) => "u8",
                    Some(65535) => "u16",
                    _ => "u64",
                }
            } else {
                "i64"
            };
            json!({"type":"integer","format":format!("agent-{width}")})
        }
        Some(Value::String(t)) if t == "number" => json!({"type":"number"}),
        Some(Value::String(t)) if t == "array" => {
            let mut out = json!({"type":"array","items":normalize(&fields["items"],&format!("{name}Item"),hints,definitions,false)});
            if fields.get("minItems") == fields.get("maxItems") {
                for k in ["minItems", "maxItems"] {
                    if let Some(n) = fields.get(k) {
                        out[k] = n.clone();
                    }
                }
            }
            out
        }
        Some(Value::String(t)) if t == "object" => {
            let extra = fields
                .get("additionalProperties")
                .cloned()
                .unwrap_or(json!({}));
            json!({"type":"object","additionalProperties":normalize(&extra,&format!("{name}Value"),hints,definitions,false)})
        }
        Some(Value::Array(types))
            if types.iter().any(|t| t == "string")
                && fields.get("format").and_then(Value::as_str) == Some("uuid") =>
        {
            json!({"anyOf":[{"type":"string","format":"uuid"},{"type":"null"}]})
        }
        Some(t) => {
            fields.clear();
            fields.insert("type".into(), t);
            Value::Object(fields)
        }
        None => json!({}),
    }
}
fn overlays(value: &Value, source: &BTreeMap<String, Value>) -> Value {
    match value {
        Value::Object(fields) => {
            let mut fields = fields.clone();
            if let Some(reference) = fields
                .get("$ref")
                .filter(|_| fields.contains_key("properties"))
                .cloned()
            {
                fields.remove("$ref");
                let name = reference.as_str().unwrap().trim_start_matches("#/$defs/");
                let mut base = source[name].as_object().unwrap().clone();
                let properties = base.get_mut("properties").unwrap().as_object_mut().unwrap();
                properties.extend(
                    fields
                        .remove("properties")
                        .unwrap()
                        .as_object()
                        .unwrap()
                        .clone(),
                );
                base.extend(fields);
                fields = base;
            }
            Value::Object(
                fields
                    .into_iter()
                    .map(|(k, v)| (k, overlays(&v, source)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(|v| overlays(v, source)).collect()),
        _ => value.clone(),
    }
}
fn serialized(field: &syn::Field) -> String {
    let mut name = field.ident.as_ref().unwrap().to_string();
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                name = meta.value()?.parse::<syn::LitStr>()?.value();
            } else if meta.input.peek(syn::Token![=]) {
                let _: syn::LitStr = meta.value()?.parse()?;
            }
            Ok(())
        });
    }
    name
}
fn reorder(fields: &mut syn::FieldsNamed, order: &Value) {
    if let Some(order) = order.as_array() {
        let mut values = fields.named.clone().into_iter().collect::<Vec<_>>();
        assert!(
            values.len() == order.len()
                && values
                    .iter()
                    .all(|f| order.iter().any(|v| v.as_str() == Some(&serialized(f)))),
            "serialization layout differs from schema: {:?} / {:?}",
            values.iter().map(serialized).collect::<Vec<_>>(),
            order
        );
        values.sort_by_key(|field| {
            order
                .iter()
                .position(|name| name.as_str() == Some(&serialized(field)))
                .unwrap_or(usize::MAX)
        });
        fields.named = values.into_iter().collect();
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema");
    let family = std::env::args().nth(1).ok_or("family required")?;
    let mut hints: Value =
        serde_json::from_slice(&std::fs::read(directory.join("projection.json"))?)?;
    if family == "enroll" {
        if let Some(orders) = hints["enrollFieldOrder"].as_object().cloned() {
            for (name, order) in orders {
                hints["fieldOrder"][name] = order;
            }
        }
    }
    let manifest_file = if family == "runtime" {
        "agent-v1.schema-manifest.json"
    } else {
        "manifest.json"
    };
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join(&family).join(manifest_file))?)?;
    let mut source = BTreeMap::<String, Value>::new();
    for entry in manifest["schemas"].as_array().ok_or("schemas")? {
        let mut schema: Value = serde_json::from_slice(&std::fs::read(
            directory
                .join(&family)
                .join(entry["file"].as_str().ok_or("file")?),
        )?)?;
        if let Some(Value::Object(defs)) = schema.as_object_mut().unwrap().remove("$defs") {
            for (name, v) in defs {
                source.insert(name, v);
            }
        }
        source.insert(entry["type"].as_str().ok_or("type")?.into(), schema);
    }
    let mut definitions = BTreeMap::new();
    for (name, v) in &source {
        let name = match name.as_str() {
            "nonNilUuid" => continue,
            "fieldType" => "ValueType",
            "fieldDefinition" => "FieldDefinition",
            "scalar" => "Scalar",
            "collection" => "CollectionDefinition",
            "fieldValue" => "FieldValue",
            "bindingProfile" => "BindingProfile",
            "grantReceipt" => "GrantReceipt",
            "activationReceipt" => "ActivationReceipt",
            "runtimeProfile" => "RuntimeProfile",
            _ => name,
        };
        let projected = normalize(&overlays(v, &source), name, &hints, &mut definitions, true);
        definitions.insert(name.into(), projected);
    }
    // Both schema families reuse the same runtime value definitions.
    let root = json!({"definitions":definitions,"type":"object","properties":{}});
    let mut settings = typify::TypeSpaceSettings::default();
    settings.with_map_type("::std::collections::BTreeMap");
    for width in ["u8", "u16", "u32", "u64", "i32", "i64"] {
        let schema =
            serde_json::from_value(json!({"type":"integer","format":format!("agent-{width}")}))?;
        settings.with_conversion(schema, width, std::iter::empty());
    }
    settings.with_conversion(
        serde_json::from_value(json!({"type":"number"}))?,
        "crate::Number",
        std::iter::empty(),
    );
    settings.with_conversion(
        serde_json::from_value(json!({"type":"string","format":"agent-secret"}))?,
        "crate::Secret",
        std::iter::empty(),
    );
    let mut types = typify::TypeSpace::new(&settings);
    types.add_root_schema(serde_json::from_value::<
        schemars_codegen::schema::RootSchema,
    >(root)?)?;
    let mut file: syn::File = syn::parse2(types.to_stream())?;
    let mut renamed = BTreeMap::<String, String>::new();
    let objects: BTreeMap<_, _> = file
        .items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Struct(s) = item {
                if let syn::Fields::Named(f) = &s.fields {
                    return Some((s.ident.to_string(), f.clone()));
                }
                None
            } else {
                None
            }
        })
        .collect();
    for item in &mut file.items {
        if let syn::Item::Struct(object) = item {
            if let syn::Fields::Named(fields) = &mut object.fields {
                reorder(fields, &hints["fieldOrder"][object.ident.to_string()]);
                for field in &mut fields.named {
                    if hints["sets"][format!("{}.{}", object.ident, serialized(field))] == true {
                        if let syn::Type::Path(path) = &field.ty {
                            if let syn::PathArguments::AngleBracketed(args) =
                                &path.path.segments.last().unwrap().arguments
                            {
                                let ty = &args.args[0];
                                field.ty = syn::parse_quote!(::std::collections::BTreeSet<#ty>);
                            }
                        }
                    }
                }
            }
        }
        if let syn::Item::Enum(enumeration) = item {
            let owner = enumeration.ident.to_string();
            for variant in &mut enumeration.variants {
                let mut label = variant.ident.to_string();
                for a in variant.attrs.iter().filter(|a| a.path().is_ident("serde")) {
                    let _ = a.parse_nested_meta(|m| {
                        if m.path.is_ident("rename") {
                            label = m.value()?.parse::<syn::LitStr>()?.value();
                        }
                        Ok(())
                    });
                }
                if let Some(payload) = hints["enumPayloads"][&owner][&label].as_str() {
                    let ty: syn::Type = syn::parse_str(payload)?;
                    variant.fields = syn::Fields::Unnamed(syn::parse_quote!((#ty)));
                } else {
                    if hints["enumFieldOrder"][&owner][&label].is_array() {
                        if let syn::Fields::Unnamed(fields) = &variant.fields {
                            if let syn::Type::Path(p) = &fields.unnamed[0].ty {
                                if let Some(f) =
                                    objects.get(&p.path.segments.last().unwrap().ident.to_string())
                                {
                                    variant.fields = syn::Fields::Named(f.clone());
                                }
                            }
                        }
                    }
                    if let syn::Fields::Named(fields) = &mut variant.fields {
                        reorder(fields, &hints["enumFieldOrder"][&owner][&label]);
                    }
                }
                if owner == "TaskPayload"
                    && matches!(label.as_str(), "TaskSpec" | "SoftwareTaskSpec")
                {
                    let ty: syn::Type = syn::parse_str(&label)?;
                    variant.fields =
                        syn::Fields::Unnamed(syn::parse_quote!((::std::boxed::Box<#ty>)));
                }
                if let Some(name) = hints["variantNames"][&owner][&label].as_str() {
                    renamed.insert(variant.ident.to_string(), name.to_owned());
                    variant.ident = syn::parse_str(name)?;
                }
            }
        }
        let field_groups: Vec<&mut syn::Field> = match item {
            syn::Item::Struct(s) => s.fields.iter_mut().collect(),
            syn::Item::Enum(e) => e
                .variants
                .iter_mut()
                .flat_map(|v| v.fields.iter_mut())
                .collect(),
            _ => vec![],
        };
        for field in field_groups {
            for attr in field
                .attrs
                .iter_mut()
                .filter(|a| a.path().is_ident("serde"))
            {
                let metas = attr.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )?;
                let retained = metas
                    .into_iter()
                    .filter(|m| !m.path().is_ident("skip_serializing_if"))
                    .collect::<syn::punctuated::Punctuated<syn::Meta, syn::Token![,]>>();
                *attr = syn::parse_quote!(#[serde(#retained)]);
            }
        }
        let (name, attrs) = match item {
            syn::Item::Struct(s) => (&s.ident, &mut s.attrs),
            syn::Item::Enum(e) => (&e.ident, &mut e.attrs),
            _ => continue,
        };
        for attr in attrs.iter_mut().filter(|a| a.path().is_ident("derive")) {
            let mut paths = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )?;
            let additions: [syn::Path; 3] = [
                syn::parse_quote!(PartialEq),
                syn::parse_quote!(Eq),
                syn::parse_quote!(schemars::JsonSchema),
            ];
            for add in additions {
                if !paths
                    .iter()
                    .any(|p| p.segments.last().unwrap().ident == add.segments.last().unwrap().ident)
                {
                    paths.push(add);
                }
            }
            *attr = syn::parse_quote!(#[derive(#paths)]);
        }
        let _ = name;
    }
    for item in &mut file.items {
        if let syn::Item::Impl(i) = item {
            if quote!(#i)
                .to_string()
                .contains("From < TaskSpec > for TaskPayload")
            {
                *i = syn::parse_quote!(impl ::std::convert::From<TaskSpec> for TaskPayload {fn from(value:TaskSpec)->Self {Self::TaskSpec(::std::boxed::Box::new(value))}});
            }
            if quote!(#i)
                .to_string()
                .contains("From < SoftwareTaskSpec > for TaskPayload")
            {
                *i = syn::parse_quote!(impl ::std::convert::From<SoftwareTaskSpec> for TaskPayload {fn from(value:SoftwareTaskSpec)->Self {Self::SoftwareTaskSpec(::std::boxed::Box::new(value))}});
            }
        }
    }
    struct Rename<'a>(&'a BTreeMap<String, String>);
    impl syn::visit_mut::VisitMut for Rename<'_> {
        fn visit_ident_mut(&mut self, ident: &mut syn::Ident) {
            if let Some(name) = self.0.get(&ident.to_string()) {
                *ident = syn::parse_str(name).unwrap();
            }
        }
    }
    syn::visit_mut::VisitMut::visit_file_mut(&mut Rename(&renamed), &mut file);
    if family == "enroll" {
        let shared = [
            "BindingProfile",
            "BindingProfileSource",
            "Capability",
            "CollectionDefinition",
            "CollectionDefinitionSource",
            "FieldDefinition",
            "Sensitivity",
            "SoftwareExecutionContext",
            "SoftwareInteractiveUser",
            "SoftwareReadCredential",
            "TaskArchitecture",
            "TaskPlatform",
            "ValueType",
        ];
        file.items.retain(|item| match item {
            syn::Item::Struct(s) => !shared.contains(&s.ident.to_string().as_str()),
            syn::Item::Enum(e) => !shared.contains(&e.ident.to_string().as_str()),
            syn::Item::Impl(i) => {
                if let syn::Type::Path(p) = &*i.self_ty {
                    !p.path
                        .segments
                        .last()
                        .is_some_and(|s| shared.contains(&s.ident.to_string().as_str()))
                } else {
                    true
                }
            }
            _ => true,
        });
        let used = quote!(#file).to_string();
        let imports = shared
            .iter()
            .filter(|name| {
                used.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|token| token == **name)
            })
            .map(|name| syn::parse_str::<syn::Ident>(name).unwrap())
            .collect::<Vec<_>>();
        file.items
            .insert(0, syn::parse_quote!(use super::runtime::{#(#imports),*};));
    }
    println!(
        "// @generated from the Agent HTTP API schemas. Do not edit.\n{}",
        quote!(#file)
            .to_string()
            .replace(":: serde_json :: Map", ":: std :: collections :: BTreeMap")
            .replace("RuntimeErrorCode", "ErrorCode")
            .replace("EnrollmentErrorCode", "ErrorCode")
            .replace("ReportBodyCode", "FailureCode")
            .replace("ReportAckIntake", "IntakeStatus")
            .replace("ReportStatusObservation", "ObservationStatus")
            .replace("ReportStatusProjection", "ProjectionStatus")
            .replace("BindingProfileSource", "ReportSource")
            .replace("FieldValueValue", "CollectedValue")
    );
    let version = manifest
        .get("wireVersion")
        .or_else(|| manifest.get("apiVersion"))
        .ok_or("version")?;
    println!("pub const API_VERSION:u8={version};");
    if family == "runtime" {
        println!("pub const WIRE_VERSION:u8={version};");
    }
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for entry in manifest["schemas"].as_array().ok_or("schemas")? {
        digest.update(std::fs::read(
            directory
                .join(&family)
                .join(entry["file"].as_str().ok_or("file")?),
        )?);
    }
    println!(
        "pub const SCHEMA_FINGERPRINT:&str=\"{:x}\";",
        digest.finalize()
    );
    println!(
        "pub const SCHEMA_MANIFEST:&str=include_str!(\"../schema/{family}/{manifest_file}\");"
    );
    Ok(())
}
