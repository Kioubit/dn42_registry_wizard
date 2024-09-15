use std::collections::HashMap;
use crate::modules::object_reader::read_registry_objects;
use crate::modules::util::BoxResult;

pub fn output(registry_root: String, v4: bool) -> BoxResult<String> {
    let json = serde_json::to_string(&get_metadata_hashmap(registry_root, v4)?)?;
    Ok(json)
}

fn get_metadata_hashmap(registry_root: String, v4: bool) -> BoxResult<HashMap<String, HashMap<String, Vec<String>>>> {
    let inetnum_path: &str = if v4 {
        "data/inetnum/"
    } else {
        "data/inet6num/"
    };

    let mut result_hash: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    let objects = read_registry_objects(registry_root, inetnum_path, false)?;
    for object in objects {
        let inetnum = object.filename.replace('_',"/");
        result_hash.insert(inetnum, object.key_value);
    }
    Ok(result_hash)
}
