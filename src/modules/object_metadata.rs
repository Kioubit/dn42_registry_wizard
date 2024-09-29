use crate::modules::object_reader::registry_objects_to_iter;
use crate::modules::util::BoxResult;
use std::collections::HashMap;

pub fn output(registry_root: String, target_folder: String,
              exclusive_fields: Option<Vec<String>>, filtered_fields: Option<Vec<String>>,
              skip_empty: bool
) -> BoxResult<String> {
    let sub_path = "data/".to_owned() + &*target_folder;
    let mut objects = HashMap::new();
    let mut registry = registry_objects_to_iter(registry_root, sub_path.as_str())?;
    if let Some(exclusive_fields) = exclusive_fields {
        registry.add_exclusive_fields(exclusive_fields);
    }
    if let Some(filtered_fields) = filtered_fields {
        registry.add_filtered_fields(filtered_fields);
    }
    for item in registry {
        let item = item?;
        if skip_empty && item.key_value.is_empty() {
            continue;
        }
        objects.insert(item.filename, item.key_value);
    }
    Ok(serde_json::to_string(&objects)?)
}