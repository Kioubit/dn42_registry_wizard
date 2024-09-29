use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use serde::Serialize;
use crate::modules::object_reader::{read_registry_objects, RegistryObject};
use crate::modules::util::BoxResult;

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

#[derive(Debug, Serialize)]
pub(crate) struct LinkedRegistryObject {
    pub category: String,
    pub object: RegistryObject,
    #[serde(serialize_with = "links_serialize")]
    pub forward_links: RefCell<Vec<Rc<LinkedRegistryObject>>>,
    #[serde(serialize_with = "links_serialize")]
    pub back_links: RefCell<Vec<Rc<LinkedRegistryObject>>>,
    pub marked: Cell<bool>,
    pub deleted: Cell<bool>,
}

fn links_serialize<S>(x: &RefCell<Vec<Rc<LinkedRegistryObject>>>, s: S) -> Result<S::Ok, S::Error>
where S: serde::Serializer {
    let link_array  = x.borrow()
        .iter()
        .map(|x| {String::from(&x.category) + "/" + &x.object.filename })
        .collect::<Vec<_>>();
    link_array.serialize(s)
}


pub fn output(registry_root: String) -> BoxResult<String> {
    let registry_schema = parse_registry_schema(registry_root.to_owned())?;
    let graph = create_registry_graph(registry_root.to_owned(), &registry_schema)?;
    Ok(serde_json::to_string(&graph)?)
}

pub(crate) type RegistryGraph = HashMap<String, Vec<Rc<LinkedRegistryObject>>>;

pub(crate) fn create_registry_graph(registry_root: String, registry_schema: &Vec<Schema>) -> BoxResult<RegistryGraph> {
    let mut object_list: HashMap<String, Vec<Rc<LinkedRegistryObject>>> = HashMap::new();

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
                marked: Cell::from(false),
                deleted: Cell::from(false),
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


                    // -------- ADD LINKS TO CURRENT OBJECT --------
                    if !Rc::ptr_eq(target_object.unwrap(), object) {
                        let mut current_obj_forward_links = object.forward_links.borrow_mut();
                        let mut found = false;
                        for obj in current_obj_forward_links.iter() {
                            if Rc::ptr_eq(obj, target_object.unwrap()) {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            current_obj_forward_links.push(target_object.unwrap().clone());
                        }
                    }
                    // ----------------------------

                    // -------- ADD BACKLINKS TO TARGET OBJECT --------
                    if !Rc::ptr_eq(object, target_object.unwrap()) {
                        let mut target_obj_back_links = target_object.unwrap().back_links.borrow_mut();
                        let mut found = false;
                        for obj in target_obj_back_links.iter() {
                            if Rc::ptr_eq(obj, object) {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            target_obj_back_links.push(object.clone());
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