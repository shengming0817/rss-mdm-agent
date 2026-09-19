use super::{fixtures, model::*};
use serde_json::value::{to_raw_value, RawValue};
use service_catalog::{FrozenCatalog, SelectedOperation};
use std::collections::BTreeMap;

// This is widget encoding only. All business rules, defaults and budgets remain in the catalog.
// Preserve numeric tokens until the catalog's bounded parser has checked precision.
pub fn select(catalog: &FrozenCatalog, draft: &Draft) -> Result<SelectedOperation> {
    let mut arguments: BTreeMap<&str, Box<RawValue>> = BTreeMap::new();
    for (name, input) in &draft.fields {
        let value = match input {
            FieldInput::Text { value } => to_raw_value(value),
            FieldInput::Boolean { value } => to_raw_value(value),
            FieldInput::SecretReference { id, revision } => {
                to_raw_value(&serde_json::json!({"id":id,"revision":revision}))
            }
            FieldInput::Integer { value } => {
                // Grammar check, never use this parsed number as the value sent to the core.
                serde_json::from_str::<serde_json::Number>(value)
                    .map_err(|_| error("input", "整数输入不是有效 JSON 数值"))?;
                RawValue::from_string(value.clone())
            }
        }
        .map_err(|_| error("input", "参数编码失败"))?;
        arguments.insert(name.as_str(), value);
    }
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Input<'a> {
        catalog: &'a service_catalog::CatalogRef,
        item_id: &'a execution_contract::Id,
        variant_id: &'a execution_contract::Id,
        arguments: BTreeMap<&'a str, Box<RawValue>>,
    }
    let bytes = serde_json::to_vec(&Input {
        catalog: &draft.catalog,
        item_id: &draft.item_id,
        variant_id: &draft.variant_id,
        arguments,
    })
    .map_err(|_| error("input", "参数编码失败"))?;
    Ok(catalog.select(&bytes, &fixtures::CATALOG_LIMITS, &fixtures::PARAMETERS)?)
}
