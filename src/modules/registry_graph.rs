use crate::modules::object_reader::{read_registry_objects, ObjectLine, RegistryObject, SimpleObjectLine};
use crate::modules::util::BoxResult;
use serde::Serialize;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;
use std::rc::{Rc, Weak};

#[derive(Debug, Serialize)]
pub(crate) struct Schema {
    pub name: String,
    pub keys: Vec<SchemaField>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SchemaField {
    pub key: String,
    pub required: bool,
    pub lookup_targets: Vec<String>,
}

pub trait ExtraDataTrait: Serialize + Debug + Default + Any {}
impl ExtraDataTrait for () {}

type LinkInformation<M, T> = ((String,String),Weak<LinkedRegistryObject<M, T>>);

#[derive(Debug, Serialize)]
pub(crate) struct LinkedRegistryObject<M: ExtraDataTrait, T: ObjectLine> {
    pub category: String,
    pub object: RegistryObject<T>,
    #[serde(serialize_with = "links_serialize")]
    forward_links: RefCell<Vec<LinkInformation<M, T>>>,
    #[serde(serialize_with = "links_serialize")]
    back_links: RefCell<Vec<LinkInformation<M, T>>>,
    #[serde(skip_serializing_if = "is_unit_type")]
    pub extra: M,
}

pub(crate) const WEAKLY_REFERENCING: [&str; 2] = ["as-set", "route-set"];

pub(crate) struct LinkIterator<'a, M: ExtraDataTrait, T: ObjectLine> {
    object: &'a LinkedRegistryObject<M, T>,
    index: usize,
    backlinks: bool,
}

impl<'a, M: ExtraDataTrait, T: ObjectLine> Iterator for LinkIterator<'a, M, T> {
    type Item = ((String, String), Rc<LinkedRegistryObject<M, T>>);

    fn next(&mut self) -> Option<Self::Item> {
        let lo = if self.backlinks {
            self.object.back_links.borrow()
        } else {
            self.object.forward_links.borrow()
        };
        let l = lo.get(self.index);
        self.index += 1;

        l.map(|(a,b)| (a.clone(), b.upgrade().unwrap()))
    }
}

impl<M: ExtraDataTrait, T: ObjectLine> LinkedRegistryObject<M, T> {
    pub fn get_back_links(&self) -> LinkIterator<M, T> {
        LinkIterator {
            object: self,
            index: 0,
            backlinks: true,
        }
    }
    pub fn get_forward_links(&self) -> LinkIterator<M, T> {
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

fn links_serialize<S, M, T>(x: &RefCell<Vec<LinkInformation<M, T>>>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
    M: ExtraDataTrait,
    T: ObjectLine,
{
    let link_array = x.borrow()
        .iter()
        .filter_map(|x| {
            x.1.upgrade().and_then(|x| {
                format!("{}/{}", x.category, x.object.filename).into()
            })
        })
        .collect::<Vec<_>>();
    link_array.serialize(s)
}

pub(crate) type RegistryGraph<M, T> = HashMap<String, Vec<Rc<LinkedRegistryObject<M, T>>>>;

pub(crate) fn create_registry_graph<M: ExtraDataTrait, T: ObjectLine<Output=T>>(registry_root: &Path, registry_schema: &Vec<Schema>, duplicate_forward_links: bool) -> BoxResult<RegistryGraph<M, T>> {
    let mut object_list: RegistryGraph<M, T> = HashMap::new();

    for schema in registry_schema {
        eprintln!("Reading {:?}", &("data/".to_owned() + &schema.name));
        let objects = read_registry_objects(registry_root, Path::new(&("data/".to_owned() + &schema.name)), false);
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
        let schema_links = &applicable_schema.keys;
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
                        .find(|x| x.object.filename.to_uppercase() == *object_key_value.get_value());
                    if target_object.is_none() {
                        continue;
                    }


                    // -------- Add links to current object --------
                    if !Rc::ptr_eq(target_object.unwrap(), object) {
                        let mut current_obj_forward_links = object.forward_links.borrow_mut();
                        let mut found = false;
                        if !duplicate_forward_links {
                            for obj in current_obj_forward_links.iter() {
                                if Rc::ptr_eq(&obj.1.upgrade().unwrap(), target_object.unwrap()) {
                                    found = true;
                                    break;
                                }
                            }
                        }
                        if !found || duplicate_forward_links {
                            current_obj_forward_links.push(((schema_key.clone(), object_key_value.get_value()), Rc::downgrade(target_object.unwrap())));
                        }
                    }
                    // ----------------------------

                    // -------- Add backlinks to target object --------
                    if !Rc::ptr_eq(object, target_object.unwrap()) {
                        let mut target_obj_back_links = target_object.unwrap().back_links.borrow_mut();
                        let mut found = false;
                        for obj in target_obj_back_links.iter() {
                            if Rc::ptr_eq(&obj.1.upgrade().unwrap(), object) {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            target_obj_back_links.push(((schema_key.clone(), object_key_value.get_value()), Rc::downgrade(object)));
                        }
                    }
                    // ----------------------------
                }
            }
        }
    }

    Ok(object_list)
}


pub(crate) fn parse_registry_schema(registry_root: &Path, exclude_registry_key: bool) -> BoxResult<Vec<Schema>> {
    let mut schemata = Vec::<Schema>::new();

    let schema_objects: Vec<RegistryObject<SimpleObjectLine>> = read_registry_objects(registry_root, Path::new("data/schema"), false)?;
    for schema_object in schema_objects {
        //if schema_object.filename == "SCHEMA-SCHEMA" {
        //    continue;
        //}

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

        let mut schema_keys: Vec<SchemaField> = Vec::new();
        for key in key_option.unwrap() {
            let key_line = key.split_whitespace().collect::<Vec<&str>>();
            let required_field = *key_line.get(1).unwrap_or(&"") == "required";

            let lookup_key_target_position = key_line.get(3).unwrap_or(&"");
            let mut lookup_key_targets : Vec<String> = Vec::new();
            if lookup_key_target_position.starts_with("lookup=") {
                lookup_key_targets = lookup_key_target_position.strip_prefix("lookup=")
                    .unwrap().split(',')
                    .filter_map(|s| s.strip_prefix("dn42."))
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>();
                if exclude_registry_key {
                    lookup_key_targets.retain(|x| x != "registry")
                }
            }

            let lookup_key = key_line.first().unwrap();
            schema_keys.push(SchemaField {
                key: lookup_key.to_string(),
                required: required_field,
                lookup_targets: lookup_key_targets,
            })
        }


        schemata.push(Schema {
            name: name.to_string(),
            keys: schema_keys,
        });
    }

    Ok(schemata)
}


pub(crate) fn link_visit<M: ExtraDataTrait, T: ObjectLine>(
    obj: &Rc<LinkedRegistryObject<M, T>>, visited: &mut Vec<Rc<LinkedRegistryObject<M, T>>>,
    to_visit: &mut Vec<Rc<LinkedRegistryObject<M, T>>>,
) {
    for link in obj.get_forward_links().chain(obj.get_back_links()) {
        let mut found = false;
        for visited in &mut *visited {
            // Do not visit a vertex twice
            if Rc::ptr_eq(&link.1, visited) {
                found = true;
                break;
            }
        }
        if found {
            continue;
        }
        // If not visited already
        visited.push(link.1.clone());
        to_visit.push(link.1.clone());
    }
}