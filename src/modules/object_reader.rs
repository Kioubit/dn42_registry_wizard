use std::collections::HashMap;
use std::fs::read_dir;
use serde::Serialize;
use crate::modules::util;
use crate::modules::util::BoxResult;


#[derive(Debug, Serialize)]
pub struct RegistryObject<> {
    pub key_value : HashMap<String,Vec<String>>,
    pub filename : String
}

pub fn read_registry_objects(registry_root: String, sub_path : &str, enumerate_only : bool) -> BoxResult<Vec<RegistryObject>>{
    let target_path = registry_root + sub_path;
    let mut objects:Vec<RegistryObject> = Vec::new();
    let dir = read_dir(target_path)?;


    for file_result in dir {
        let file = file_result?.path();
        let filename = file.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned();
        if filename == "." || filename == ".."  {
            continue;
        }

        let mut map: HashMap<String,Vec<String>> = HashMap::new();

        if !enumerate_only {
            let lines = util::read_lines(&file)?;
            for line in lines {
                if let Some(result) = line?.split_once(':') {
                    let obj_key  = result.0.trim_end();
                    if !map.contains_key(obj_key) {
                        map.insert(obj_key.to_string(), Vec::new());
                    }
                    let key = map.get_mut(obj_key).unwrap();
                    key.push(result.1.trim().to_string())
                }
            }
        }
        let final_object = RegistryObject{
            key_value : map,
            filename
        };
        objects.push(final_object);
    }

    Ok(objects)
}