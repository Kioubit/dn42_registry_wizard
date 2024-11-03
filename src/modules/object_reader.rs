use crate::modules::util;
use crate::modules::util::BoxResult;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::read_dir;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};


pub(in crate::modules) trait ObjectLine: Debug + Serialize + Clone {
    fn append_to_last(key: &mut Vec<Self>, value: &str);
    fn push_line(key: &mut Vec<Self>, value: String, line: usize);

    fn get_line_value(&self) -> String;
}

pub(in crate::modules) type OrderedObjectLine = (usize, String);
pub(in crate::modules) type SimpleObjectLine = String;

#[derive(Debug, Serialize, Clone)]
pub(in crate::modules) struct RegistryObject<T>
where
    T: ObjectLine,
{
    pub key_value: HashMap<String, Vec<T>>,
    pub filename: String,
}

pub(in crate::modules) struct RegistryObjectIterator<T: ObjectLine> {
    _marker: PhantomData<T>,
    paths: Vec<(String, PathBuf)>,
    filename_filter: Vec<String>,
    exclusive_fields: RefCell<Option<Vec<String>>>,
    filtered_fields: RefCell<Option<Vec<String>>>,
    enumerate_only: bool,
}

impl<T: ObjectLine> RegistryObjectIterator<T> {
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

impl<T: ObjectLine> Iterator for RegistryObjectIterator<T> {
    type Item = BoxResult<RegistryObject<T>>;
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

pub(in crate::modules) fn registry_objects_to_iter<T: ObjectLine>(registry_root: &Path, sub_path: &Path) -> BoxResult<RegistryObjectIterator<T>> {
    let paths = get_object_paths(registry_root, sub_path)?;
    Ok(RegistryObjectIterator {
        _marker: Default::default(),
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

pub(in crate::modules) fn read_registry_objects<T: ObjectLine>(registry_root: &Path, sub_path: &Path, enumerate_only: bool) -> BoxResult<Vec<RegistryObject<T>>> {
    let paths = get_object_paths(registry_root, sub_path)?;

    let mut objects = Vec::<RegistryObject<T>>::new();

    for path in paths {
        let map = if enumerate_only {
            HashMap::<String, Vec<T>>::new()
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

pub(in crate::modules) fn read_registry_object_kv<T: ObjectLine>(path: &Path) -> BoxResult<HashMap<String, Vec<T>>> {
    read_registry_object_kv_filtered(path, &None, &None)
}

impl ObjectLine for SimpleObjectLine {
    fn append_to_last(key: &mut Vec<Self>, value: &str) {
        key.last_mut().unwrap().push_str(value)
    }

    fn push_line(key: &mut Vec<Self>, value: String, _: usize) {
        key.push(value)
    }
    fn get_line_value(&self) -> String {
        self.clone()
    }
}

impl ObjectLine for OrderedObjectLine {
    fn append_to_last(key: &mut Vec<Self>, value: &str) {
        let last = key.last_mut().unwrap();
        last.1.push_str(value);
    }

    fn push_line(key: &mut Vec<Self>, value: String, line: usize) {
        key.push((line, value))
    }
    fn get_line_value(&self) -> String {
        self.1.clone()
    }
}


pub(in crate::modules) fn read_registry_object_kv_filtered<T: ObjectLine>(path: &Path, exclusive_fields: &Option<Vec<String>>,
                                                                          filtered_fields: &Option<Vec<String>>)
                                                                          -> BoxResult<HashMap<String, Vec<T>>> {
    let mut map: HashMap<String, Vec<T>> = HashMap::new();
    let lines = util::read_lines(path)?;
    let mut last_obj_key: Option<String> = None;
    for (no, line) in lines.into_iter().enumerate() {
        let line = line?;
        let split_result = line.split_once(':');
        if !line.starts_with(' ') && split_result.is_some() {
            let result = split_result.unwrap();
            last_obj_key = None;
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
            T::push_line(key, result.1.trim().to_string(), no);
            last_obj_key = Some(obj_key.to_string());
        } else if let Some(ref last_obj_key) = last_obj_key {
            // Handle multi-line
            let key = map.get_mut(last_obj_key).unwrap();
            T::append_to_last(key, "\n");
            if !line.starts_with('+') {
                T::append_to_last(key, line.trim());
            }
        }
    }

    Ok(map)
}


#[allow(dead_code)]
pub(in crate::modules) fn filter_objects_source(objects: &mut Vec<RegistryObject<SimpleObjectLine>>, source: String) {
    objects.retain(|obj| obj.key_value.get("source").is_some_and(|x| x.first() == Some(&source)));
}