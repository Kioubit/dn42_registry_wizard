use crate::modules::registry_graph::{create_registry_graph, link_visit, parse_registry_schema, ExtraDataTrait, LinkInfoNone, LinkInfoSchemaKey, LinkedRegistryObject, WEAKLY_REFERENCING};
use crate::modules::registry_graphviz::create_graphviz;
use crate::modules::util::BoxResult;
use serde::Serialize;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use crate::modules::object_reader::SimpleObjectLine;

pub fn output_list(registry_root: &Path, obj_type: Option<String>, object_name: Option<String>, graphviz: bool) -> BoxResult<String> {
    let registry_schema = parse_registry_schema(registry_root, true)?;
    let graph = create_registry_graph(registry_root, &registry_schema, false,false)?;
    match obj_type {
        None => {
            if graphviz {
                let full = graph.iter().flat_map(|vec| vec.1).cloned().collect::<Vec<_>>();
                Ok(create_graphviz(full, None)?)
            } else {
                Ok(serde_json::to_string(&graph)?)
            }
        }
        Some(s) => {
            match object_name {
                None => {
                    let v = graph.get(&s).ok_or("object type not found")?;
                    if graphviz {
                        Ok(create_graphviz(v.to_vec(), None)?)
                    } else {
                        Ok(serde_json::to_string(v)?)
                    }
                }
                Some(n) => {
                    if graphviz {
                        return Err("Cannot use the graphviz option in combination with object_name".into());
                    }
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

pub fn output_related(registry_root: &Path, obj_type: String,
                      obj_name: String, enforce_mnt_by: Option<String>, only_related_to_mnt: Option<String>,
                      graphviz: bool,
) -> BoxResult<String> {
    let schema = parse_registry_schema(registry_root, true)?;
    let graph = create_registry_graph::<(), SimpleObjectLine, LinkInfoSchemaKey>(registry_root, &schema, false, false)?;
    let t_obj = graph.get(&obj_type).ok_or("specified object type not found")?
        .iter().find(|x| x.object.filename == obj_name)
        .ok_or("specified obj_name not found")?;

    let mut visited: Vec<Rc<LinkedRegistryObject<(), SimpleObjectLine, LinkInfoSchemaKey>>> = Vec::new();
    let mut to_visit: Vec<Rc<LinkedRegistryObject<(), SimpleObjectLine, LinkInfoSchemaKey>>> = Vec::new();
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
        link_visit(&obj, &mut visited, &mut to_visit);
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


pub fn output_path(registry_root: &Path, src_type: String, tgt_type: String,
                   src_name: String, tgt_name: String) -> BoxResult<String> {
    #[derive(Default, Debug, Serialize)]
    struct ParentInfo(RefCell<Option<Rc<LinkedRegistryObject<ParentInfo, SimpleObjectLine, LinkInfoNone>>>>);
    impl ExtraDataTrait for ParentInfo {}

    let schema = parse_registry_schema(registry_root, true)?;
    let graph = create_registry_graph(registry_root, &schema, false, false)?;
    let s_obj = graph.get(&src_type)
        .ok_or("specified src object type not found")?
        .iter().find(|x| x.object.filename == src_name)
        .ok_or("specified src_name not found")?;
    let t_obj = graph.get(&tgt_type)
        .ok_or("specified tgt object type not found")?
        .iter().find(|x| x.object.filename == tgt_name)
        .ok_or("specified tgt_name not found")?;

    // Perform a breadth-first search
    let mut visited: Vec<Rc<LinkedRegistryObject<ParentInfo,SimpleObjectLine, LinkInfoNone>>> = Vec::new();
    let mut to_visit: Vec<Rc<LinkedRegistryObject<ParentInfo, SimpleObjectLine, LinkInfoNone>>> = Vec::new();
    visited.push(s_obj.clone());
    to_visit.push(s_obj.clone());
    let mut found = false;
    while !to_visit.is_empty() {
        let obj = to_visit.remove(0);
        if WEAKLY_REFERENCING.contains(&obj.category.as_str()) {
            continue;
        }
        if &obj.category == "aut-num" && obj.object.filename == "AS0" {
            // Special case
            continue;
        }

        if Rc::ptr_eq(&obj, t_obj) {
            found = true;
            break;
        }
        let mut temp_to_visit = Vec::new();
        link_visit(&obj, &mut visited, &mut temp_to_visit);
        for v_obj in &temp_to_visit {
            v_obj.extra.0.replace(Some(obj.clone()));
        }
        to_visit.append(&mut temp_to_visit);
    }

    if !found {
        return Err("A path between the specified objects was not found".into());
    }

    let mut rev_path = Vec::new();
    rev_path.push(t_obj.clone());
    let mut cur_obj = t_obj.clone();
    loop {
        let parent = cur_obj.extra.0.take();
        if let Some(parent) = parent {
            cur_obj = parent;
            rev_path.push(cur_obj.clone());
        } else {
            break;
        }
    }

    Ok(rev_path.iter().rev()
        .map(|x| format!("{}/{}", x.category, x.object.filename.clone()))
        .collect::<Vec<String>>().join(" > "))
}