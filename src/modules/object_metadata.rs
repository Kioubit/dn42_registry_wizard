use crate::modules::object_reader::read_registry_objects;
use crate::modules::util::BoxResult;

pub fn output(registry_root: String, target_folder: String) -> BoxResult<String> {
    let sub_path = "data/".to_owned() + &*target_folder;
    let result =
        read_registry_objects(registry_root, sub_path.as_str(), false)?;
    Ok(serde_json::to_string(&result)?)
}