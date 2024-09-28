use std::collections::HashMap;
use std::fs::read_dir;
use std::path::PathBuf;
use serde::Serialize;
use crate::modules::util;
use crate::modules::util::BoxResult;


#[derive(Debug, Serialize)]
pub struct RegistryObject {
    pub key_value: HashMap<String, Vec<String>>,
    pub filename: String,
}

pub struct RegistryObjectIterator {
    paths: Vec<(String, PathBuf)>,
    filename_filter: Vec<String>,
}

impl RegistryObjectIterator {
    pub fn add_filename_filter(&mut self, filter: &str) -> &Self {
        self.filename_filter.push(filter.to_owned());
        self
    }
}

impl Iterator for RegistryObjectIterator {
    type Item = BoxResult<RegistryObject>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut path: (String, PathBuf);
        loop {
            if self.paths.is_empty() { return None; }
            path = self.paths.pop().unwrap();
            if !self.filename_filter.is_empty() &&
                self.filename_filter.iter().any(|x| path.0.contains(x)) {
                continue;
            }
            break;
        }
        let obj = read_registry_object_kv(path.1);
        match obj {
            Ok(obj) => {
                Some(Ok(RegistryObject {
                    key_value: obj,
                    filename: path.0,
                }))
            }
            Err(err) => { Some(Err(err)) }
        }
    }
}

pub fn registry_objects_to_iter(registry_root: String, sub_path: &str) -> BoxResult<RegistryObjectIterator> {
    let paths = get_object_paths(registry_root, sub_path)?;
    Ok(RegistryObjectIterator { paths, filename_filter: vec![] })
}

fn get_object_paths(registry_root: String, sub_path: &str) -> BoxResult<Vec<(String, PathBuf)>> {
    let target_path = registry_root + sub_path;
    let dir = read_dir(target_path)?;
    let mut paths = Vec::<(String, PathBuf)>::new();
    for file_result in dir {
        let file = file_result?.path();
        let filename = file.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned();
        if filename == "." || filename == ".." || filename.is_empty() {
            continue;
        }
        paths.push((filename, file));
    }
    Ok(paths)
}

pub fn read_registry_objects(registry_root: String, sub_path: &str, enumerate_only: bool) -> BoxResult<Vec<RegistryObject>> {
    let paths = get_object_paths(registry_root, sub_path)?;

    let mut objects = Vec::<RegistryObject>::new();

    for path in paths {
        let map = if enumerate_only {
            HashMap::<String, Vec<String>>::new()
        } else {
            read_registry_object_kv(path.1)?
        };

        objects.push(RegistryObject {
            key_value: map,
            filename: path.0,
        });
    }

    Ok(objects)
}

pub fn read_registry_object_kv(path: PathBuf) -> BoxResult<HashMap<String, Vec<String>>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let lines = util::read_lines(&path)?;
    for line in lines {
        if let Some(result) = line?.split_once(':') {
            let obj_key = result.0.trim_end();
            if !map.contains_key(obj_key) {
                map.insert(obj_key.to_string(), Vec::new());
            }
            let key = map.get_mut(obj_key).unwrap();
            key.push(result.1.trim().to_string())
        }
    }

    Ok(map)
}

pub fn filter_objects_source(objects: &mut Vec<RegistryObject>, source: String) {
    objects.retain(|obj| obj.key_value.get("source").is_some_and(|x| x.first() == Some(&source)));
}