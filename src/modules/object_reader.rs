use crate::modules::util;
use crate::modules::util::BoxResult;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::read_dir;
use std::path::{Path, PathBuf};


#[derive(Debug, Serialize)]
pub struct RegistryObject {
    pub key_value: HashMap<String, Vec<String>>,
    pub filename: String,
}

pub struct RegistryObjectIterator {
    paths: Vec<(String, PathBuf)>,
    filename_filter: Vec<String>,
    exclusive_fields: RefCell<Option<Vec<String>>>,
    filtered_fields: RefCell<Option<Vec<String>>>,
    enumerate_only: bool,
}

impl RegistryObjectIterator {
    pub fn set_enumerate_only(&mut self, state: bool) -> &Self {
        self.enumerate_only = state;
        self
    }
    pub fn add_filename_filter(&mut self, filter: &str) -> &Self {
        self.filename_filter.push(filter.to_owned());
        self
    }
    pub fn add_exclusive_fields(&mut self, list: Vec<String>) -> &Self {
        self.exclusive_fields.replace(Some(list));
        self
    }
    pub fn add_filtered_fields(&mut self, list: Vec<String>) -> &Self {
        self.filtered_fields.replace(Some(list));
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

        if self.enumerate_only {
            return Some(Ok(RegistryObject {
                key_value: Default::default(),
                filename: path.0,
            }));
        }

        let obj = read_registry_object_kv_filtered(&path.1, &self.exclusive_fields.borrow(), &self.filtered_fields.borrow());
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

pub fn registry_objects_to_iter(registry_root: &Path, sub_path: &Path) -> BoxResult<RegistryObjectIterator> {
    let paths = get_object_paths(registry_root, sub_path)?;
    Ok(RegistryObjectIterator {
        paths,
        filename_filter: vec![],
        exclusive_fields: RefCell::new(None),
        filtered_fields: RefCell::new(None),
        enumerate_only: false,
    })
}

fn get_object_paths(registry_root: &Path, sub_path: &Path) -> BoxResult<Vec<(String, PathBuf)>> {
    let target_path = registry_root.join(sub_path);
    let dir = read_dir(&target_path)
        .map_err(|e| format!("Error opening directory {}: {}", target_path.display(), e))?;
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

pub fn read_registry_objects(registry_root: &Path, sub_path: &Path, enumerate_only: bool) -> BoxResult<Vec<RegistryObject>> {
    let paths = get_object_paths(registry_root, sub_path)?;

    let mut objects = Vec::<RegistryObject>::new();

    for path in paths {
        let map = if enumerate_only {
            HashMap::<String, Vec<String>>::new()
        } else {
            read_registry_object_kv(&path.1)?
        };

        objects.push(RegistryObject {
            key_value: map,
            filename: path.0,
        });
    }

    Ok(objects)
}

pub fn read_registry_object_kv(path: &Path) -> BoxResult<HashMap<String, Vec<String>>> {
    read_registry_object_kv_filtered(path, &None, &None)
}

pub fn read_registry_object_kv_filtered(path: &Path, exclusive_fields: &Option<Vec<String>>,
                                        filtered_fields: &Option<Vec<String>>)
                                        -> BoxResult<HashMap<String, Vec<String>>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let lines = util::read_lines(path)?;
    for line in lines {
        if let Some(result) = line?.split_once(':') {
            let obj_key = result.0.trim_end();

            if let Some(ref f) = exclusive_fields {
                if !f.contains(&obj_key.to_string()) {
                    continue;
                }
            }

            if let Some(ref f) = filtered_fields {
                if f.contains(&obj_key.to_string()) {
                    continue;
                }
            }

            if !map.contains_key(obj_key) {
                map.insert(obj_key.to_string(), Vec::new());
            }
            let key = map.get_mut(obj_key).unwrap();
            key.push(result.1.trim().to_string())
        }
    }

    Ok(map)
}

#[allow(dead_code)]
pub fn filter_objects_source(objects: &mut Vec<RegistryObject>, source: String) {
    objects.retain(|obj| obj.key_value.get("source").is_some_and(|x| x.first() == Some(&source)));
}