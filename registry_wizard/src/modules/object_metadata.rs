use crate::modules::object_reader::{registry_objects_to_iter, RegistryObjectIterator, SimpleObjectLine};
use crate::modules::util::BoxResult;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn output(registry_root: &Path, object_type: &str,
              exclusive_fields: Option<Vec<String>>, filtered_fields: Option<Vec<String>>,
              skip_empty: bool
) -> BoxResult<String> {
    let sub_path = PathBuf::from("data/").join(Path::new(object_type));
    let mut objects = HashMap::new();
    let mut registry: RegistryObjectIterator<SimpleObjectLine> = registry_objects_to_iter(registry_root, &sub_path)?;
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