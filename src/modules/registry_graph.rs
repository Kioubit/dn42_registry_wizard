use crate::modules::object_reader::{read_registry_objects, RegistryObject};
use crate::modules::util::BoxResult;
use serde::Serialize;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::{Rc, Weak};
use crate::modules::registry_graphviz::create_graphviz;

#[derive(Debug)]
pub(crate) struct Schema {
    pub name: String,
    pub lookup_keys: Vec<SchemaField>,
}

#[derive(Debug)]
pub(crate) struct SchemaField {
    pub key: String,
    pub required: bool,
    pub lookup_targets: Vec<String>,
}

pub trait ExtraDataTrait: Serialize + Debug + Default + Any {}
impl ExtraDataTrait for () {}

#[derive(Debug, Serialize)]
pub(crate) struct LinkedRegistryObject<M: ExtraDataTrait> {
    pub category: String,
    pub object: RegistryObject,
    #[serde(serialize_with = "links_serialize")]
    forward_links: RefCell<Vec<Weak<LinkedRegistryObject<M>>>>,
    #[serde(serialize_with = "links_serialize")]
    back_links: RefCell<Vec<Weak<LinkedRegistryObject<M>>>>,
    #[serde(skip_serializing_if = "is_unit_type")]
    pub extra: M,
}

pub(crate) const WEAKLY_REFERENCING: [&str; 2] = ["as-set", "route-set"];

pub(crate) struct LinkIterator<'a, M: ExtraDataTrait> {
    object: &'a LinkedRegistryObject<M>,
    index: usize,
    backlinks: bool,
}

impl<'a, M: ExtraDataTrait> Iterator for LinkIterator<'a, M> {
    type Item = Rc<LinkedRegistryObject<M>>;

    fn next(&mut self) -> Option<Self::Item> {
        let lo = if self.backlinks {
            self.object.back_links.borrow()
        } else { 
            self.object.forward_links.borrow()
        };
        let l = lo.get(self.index);
        self.index += 1;

        l.map(|x| x.upgrade().unwrap())
    }
}

impl<M: ExtraDataTrait> LinkedRegistryObject<M> {
    pub fn get_back_links(&self) -> LinkIterator<M> {
        LinkIterator {
            object: self,
            index: 0,
            backlinks: true,
        }
    }
    pub fn get_forward_links(&self) -> LinkIterator<M> {
        LinkIterator {
            object: self,
            index: 0,
            backlinks: false,
        }
    }
}

fn is_unit_type<T: Any>(_: &T) -> bool {
    std::any::TypeId::of::<T>() == std::any::TypeId::of::<()>()
}

fn links_serialize<S, M>(x: &RefCell<Vec<Weak<LinkedRegistryObject<M>>>>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    M: ExtraDataTrait,
{
    let link_array = x.borrow()
        .iter()
        .filter_map(|x| {
            x.upgrade().and_then(|x| {
                format!("{}/{}", x.category, x.object.filename).into()
            })
        })
        .collect::<Vec<_>>();
    link_array.serialize(s)
}

pub fn output_list(registry_root: String, obj_type: Option<String>, object_name: Option<String>) -> BoxResult<String> {
    let registry_schema = parse_registry_schema(registry_root.to_owned())?;
    let graph = create_registry_graph::<()>(registry_root.to_owned(), &registry_schema)?;
    match obj_type {
        None => {
            Ok(serde_json::to_string(&graph)?)
        }
        Some(s) => {
            match object_name {
                None => {
                    Ok(serde_json::to_string(&graph.get(&s).ok_or("object type not found")?)?)
                }
                Some(n) => {
                    let r = &graph.get(&s)
                        .ok_or("object type not found")?
                        .iter().find(|x| x.object.filename == n)
                        .ok_or("object by name not found");
                    Ok(serde_json::to_string(r)?)
                }
            }
        }
    }
}

pub fn output_related(registry_root: String, obj_type: String,
                      obj_name: String, enforce_mnt_by: Option<String>, only_related_to_mnt: Option<String>,
                      graphviz: bool
) -> BoxResult<String> {
    let schema = parse_registry_schema(registry_root.to_owned())?;
    let graph = create_registry_graph::<()>(registry_root, &schema)?;
    let t_obj = graph.get(&obj_type).ok_or("specified object type not found")?
        .iter().find(|x| x.object.filename == obj_name)
        .ok_or("specified obj_name not found")?;

    let mut visited: Vec<Rc<LinkedRegistryObject<()>>> = Vec::new();
    let mut to_visit: Vec<Rc<LinkedRegistryObject<()>>> = Vec::new();
    visited.push(t_obj.clone());
    to_visit.push(t_obj.clone());
    while let Some(obj) = to_visit.pop() {
        if WEAKLY_REFERENCING.contains(&obj.category.as_str()) {
            continue;
        }
        if let Some(ref target) = only_related_to_mnt {
            if let Some(m) = obj.object.key_value.get("mnt-by") {
                if m.iter().any(|x| x != target) {
                    continue;
                }
            }
        }
        link_recurse(&obj, &mut visited, &mut to_visit);
    }
    if let Some(ref target) = enforce_mnt_by {
        visited.retain(|v| {
            if let Some(m) = v.object.key_value.get("mnt-by") {
                if m.iter().any(|x| x != target) {
                    return false;
                }
            }
            true
        });
    }
    
    if graphviz {
        let mnt = if only_related_to_mnt.is_some() {
            only_related_to_mnt
        } else if enforce_mnt_by.is_some() { 
            enforce_mnt_by
        } else { 
            None
        };
        return create_graphviz(visited.clone(), mnt);
    }
    
    let result: Vec<_> = visited.iter()
        .map(|x| format!("{}/{}", x.category, x.object.filename))
        .collect();
    Ok(serde_json::to_string(&result)?)
}

pub(crate) type RegistryGraph<M> = HashMap<String, Vec<Rc<LinkedRegistryObject<M>>>>;

pub(crate) fn create_registry_graph<M: ExtraDataTrait>(registry_root: String, registry_schema: &Vec<Schema>) -> BoxResult<RegistryGraph<M>> {
    let mut object_list: RegistryGraph<M> = HashMap::new();

    for schema in registry_schema {
        eprintln!("Reading {:?}", &("data/".to_owned() + &schema.name));
        let objects = read_registry_objects(registry_root.clone(), &("data/".to_owned() + &schema.name), false);
        if objects.is_err() {
            eprintln!("Error accessing directory referred to by schema: {}", schema.name.clone());
            continue;
        }
        for object in objects? {
            let x = object_list.entry(schema.name.clone()).or_default();
            x.push(Rc::from(LinkedRegistryObject {
                category: schema.name.clone(),
                object,
                forward_links: RefCell::new(vec![]),
                back_links: RefCell::new(vec![]),
                extra: Default::default(),
            }))
        }
    }


    // Establish links
    for object in object_list.values().flatten() {
        // For each object regardless of category

        let applicable_schema = registry_schema.iter().find(|x| x.name == *object.category).unwrap();
        let schema_links = &applicable_schema.lookup_keys;
        for schema_link in schema_links {
            let schema_link_targets = &schema_link.lookup_targets;
            let schema_key = &schema_link.key;
            // For each schema_key described in the applicable schema linking via 'lookup=' to a schema_link_target

            // Try to find the schema_key in the object
            let object_key_values = object.object.key_value.get(schema_key);
            if object_key_values.is_none() {
                // Not found
                continue;
            }

            // For each found schema key in the object (for instance mnt-by)
            for object_key_value in object_key_values.unwrap() {
                // Get all 'lookup=' targets
                for possible_category in schema_link_targets {
                    let t_category = &object_list.get(possible_category);
                    if t_category.is_none() {
                        eprintln!("Error: unknown category \"{}\"", possible_category);
                        continue;
                    }
                    let target_object = t_category.unwrap()
                        .iter()
                        .find(|x| x.object.filename.to_uppercase() == *object_key_value);
                    if target_object.is_none() {
                        continue;
                    }


                    // -------- Add links to current object --------
                    if !Rc::ptr_eq(target_object.unwrap(), object) {
                        let mut current_obj_forward_links = object.forward_links.borrow_mut();
                        let mut found = false;
                        for obj in current_obj_forward_links.iter() {
                            if Rc::ptr_eq(&obj.upgrade().unwrap(), target_object.unwrap()) {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            current_obj_forward_links.push(Rc::downgrade(target_object.unwrap()));
                        }
                    }
                    // ----------------------------

                    // -------- Add backlinks to target object --------
                    if !Rc::ptr_eq(object, target_object.unwrap()) {
                        let mut target_obj_back_links = target_object.unwrap().back_links.borrow_mut();
                        let mut found = false;
                        for obj in target_obj_back_links.iter() {
                            if Rc::ptr_eq(&obj.upgrade().unwrap(), object) {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            target_obj_back_links.push(Rc::downgrade(object));
                        }
                    }
                    // ----------------------------
                }
            }
        }
    }

    Ok(object_list)
}


pub(crate) fn parse_registry_schema(registry_root: String) -> BoxResult<Vec<Schema>> {
    let mut schemata = Vec::<Schema>::new();

    let schema_objects = read_registry_objects(registry_root, "data/schema", false)?;
    for schema_object in schema_objects {
        if schema_object.filename == "SCHEMA-SCHEMA" {
            continue;
        }

        let mut name_vec = schema_object.key_value.get("dir-name");
        if name_vec.is_none() {
            name_vec = schema_object.key_value.get("ref");
            if name_vec.is_none() {
                eprintln!("Error: schema object missing 'ref' key: {}", schema_object.filename);
                continue;
            }
        }

        let name_vec_first = name_vec.unwrap().first().unwrap();
        let name = match name_vec_first.strip_prefix("dn42.") {
            Some(x) => x,
            None => name_vec_first,
        };

        let key_option = schema_object.key_value.get("key");
        if key_option.is_none() {
            eprintln!("Error: schema object missing 'key' key: {}", schema_object.filename);
            continue;
        }

        let mut lookup_targets: Vec<SchemaField> = Vec::new();
        for key in key_option.unwrap() {
            let key_line = key.split_whitespace().collect::<Vec<&str>>();
            let required_field = *key_line.get(1).unwrap_or(&"") == "required";

            let lookup_key_target_position = key_line.get(3).unwrap_or(&"");
            if !lookup_key_target_position.starts_with("lookup=") {
                continue;
            }
            let lookup_key_targets = lookup_key_target_position.strip_prefix("lookup=")
                .unwrap().split(',')
                .filter_map(|s| s.strip_prefix("dn42."))
                .filter(|x| *x != "registry")
                .map(|x| x.to_string()).collect::<Vec<String>>();
            let lookup_key = key_line.first().unwrap();
            lookup_targets.push(SchemaField {
                key: lookup_key.to_string(),
                required: required_field,
                lookup_targets: lookup_key_targets,
            })
        }


        schemata.push(Schema {
            name: name.to_string(),
            lookup_keys: lookup_targets,
        });
    }

    Ok(schemata)
}


pub(crate) fn link_recurse<M: ExtraDataTrait>(obj: &Rc<LinkedRegistryObject<M>>,
                                              visited: &mut Vec<Rc<LinkedRegistryObject<M>>>,
                                              to_visit: &mut Vec<Rc<LinkedRegistryObject<M>>>) {
    for link in obj.get_forward_links().chain(obj.get_back_links()) {
        let mut found = false;
        for visited in &mut *visited {
            // Do not visit a vertex twice
            if Rc::ptr_eq(&link, visited) {
                found = true;
                break;
            }
        }
        if found {
            continue;
        }
        // If not visited already
        visited.push(link.clone());
        to_visit.push(link.clone());
    }
}