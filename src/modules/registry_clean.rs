use std::cell::Cell;
use std::rc::Rc;
use serde::Serialize;
use crate::modules::registry_graph::{create_registry_graph, parse_registry_schema, LinkedRegistryObject, ExtraDataTrait};
use crate::modules::util;
use crate::modules::util::{BoxResult, EitherOr};


#[derive(Debug, Serialize, Default)]
struct MetaData {
    marked: Cell<bool>,
    deleted: Cell<bool>,
}
impl ExtraDataTrait for MetaData {}

pub fn output(registry_root: String, mnt_input: EitherOr<String, String>, with_subgraph_check: bool) -> BoxResult<String> {
    if !with_subgraph_check {
        eprintln!("Warning: Subgraph check has been disabled")
    }
    let weakly_referencing: [&str; 2] = ["as-set", "route-set"];

    let mut output = String::new();

    let mnt_raw_list = match mnt_input {
        EitherOr::A(file) => {
            util::read_lines(file)?.map_while(Result::ok).collect::<Vec<String>>().join("\n")
        }
        EitherOr::B(list) => {
            list
        }
    };
    let mnt_list = mnt_raw_list.split(",").collect::<Vec<&str>>();
    let only_one_mnt = matches!(mnt_list.len(), 1);

    let registry_schema = parse_registry_schema(registry_root.to_owned())?;

    let graph = create_registry_graph::<MetaData>(registry_root.to_owned(), &registry_schema)?;
    let mntner = graph.get("mntner").ok_or("mntner not found")?;

    // Assuming the registry objects form an undirected graph which is a superset of many disconnected sub-graphs
    // Mark all mntner vertices to delete
    eprintln!("Analyzing dependency graph (1/6)");
    for mnt in mntner {
        let mnt = mnt.clone();
        if mnt_list.contains(&&*mnt.object.filename) {
            mnt.extra.marked.set(true);
        }
    }

    eprintln!("Analyzing dependency graph (2/6)");
    // For every *unmarked* vertex
    for mnt in mntner {
        if mnt.extra.marked.get() {
            continue;
        }
        // Recursively follow each path while keeping track of visited vertices
        let mut visited: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        visited.push(mnt.clone());
        to_visit.push(mnt.clone());

        while let Some(obj) = to_visit.pop() {
            if weakly_referencing.contains(&obj.category.as_str()) {
                continue;
            }

            // If a *marked* mntner vertex is encountered, unmark it and flag it for manual review
            if obj.extra.marked.get() {
                eprintln!("Manual review: {} (First conflict with active MNT: {})",
                          obj.object.filename, mnt.object.filename
                );
                if only_one_mnt {
                    return Err("Manual review needed".into());
                }
                obj.extra.marked.set(false);
            }

            link_recurse(&obj, &mut visited, &mut to_visit);
        }
    }


    eprintln!("Analyzing dependency graph (3/6)");
    // For every *still marked* mntner vertex: Recursively delete all vertices
    // Recursively follow each path while keeping track of visited vertices
    for mnt in mntner {
        if !mnt.extra.marked.get() {
            continue;
        }
        let mut visited: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        visited.push(mnt.clone());
        to_visit.push(mnt.clone());

        while let Some(obj) = to_visit.pop() {
            if weakly_referencing.contains(&obj.category.as_str()) {
                continue;
            }
            if obj.extra.deleted.get() {
                continue;
            }
            obj.extra.deleted.set(true);
            output.push_str(&format!("rm 'data/{}/{}'\n", obj.category, obj.object.filename));

            link_recurse(&obj, &mut visited, &mut to_visit);
        }
    }

    eprintln!("Analyzing dependency graph (4/6)");
    // Check if weakly referenced objects have dangling references
    for w in weakly_referencing {
        let empty_vec = vec![];
        let w_list: Vec<_> = graph.get(w)
            .unwrap_or(&empty_vec)
            .iter().collect();
        for w_item in w_list {
            let mut found = false;
            for reference in w_item.back_links.borrow().iter()
                .chain(w_item.forward_links.borrow().iter()) {
                if reference.upgrade().unwrap().extra.deleted.get() {
                    continue;
                }
                found = true;
            }
            if !found {
                w_item.extra.deleted.set(true);
                output.push_str(&format!("rm 'data/{}/{}'\n", w_item.category, w_item.object.filename));
                continue;
            }
        }
    }

    eprintln!("Analyzing dependency graph (5/6)");
    // Check for remaining dangling references
    for item in graph.values().flatten() {
        if item.extra.deleted.get() {
            continue;
        }

        let mut has_links = false;
        for link in item.back_links.borrow().iter()
            .chain(item.forward_links.borrow().iter()) {
            if !link.upgrade().unwrap().extra.deleted.get() {
                has_links = true;
                continue;
            }
            output.push_str(&format!("sed -i '/{}/d' 'data/{}/{}'\n", link.upgrade().unwrap().object.filename, item.category, item.object.filename));
        }

        if !has_links {
            item.extra.deleted.set(true);
            output.push_str(&format!("rm 'data/{}/{}'\n", item.category, item.object.filename));
            continue;
        }
    }

    eprintln!("Analyzing dependency graph (6/6)");
    // Final pass
    // Check if all required lookup keys are present (important for weakly referencing objects)
    for item in graph.values().flatten() {
        if item.extra.deleted.get() {
            continue;
        }

        let applicable_schema = &registry_schema.iter().find(|x| x.name == item.category);
        if applicable_schema.is_none() {
            eprintln!("Warning: can't find schema for category '{}'", item.category);
            continue;
        }
        let required_categories = applicable_schema.unwrap()
            .lookup_keys.iter()
            .filter(|x| x.required)
            .flat_map(|x| x.lookup_targets.iter())
            .collect::<Vec<_>>();
        let mut required_category_missing = false;
        for required_category in required_categories {
            if *required_category == item.category {
                // We have that category
                continue;
            }
            if !item.forward_links.borrow().iter()
                .filter(|x| !x.upgrade().unwrap().extra.deleted.get())
                .any(|x| x.upgrade().unwrap().category == *required_category) {
                // If we don't find a link with the required category
                required_category_missing = true;
                break;
            }
        }
        if required_category_missing {
            item.extra.deleted.set(true);
            output.push_str(&format!("rm 'data/{}/{}'\n", item.category, item.object.filename));
            continue;
        }
    }

    if !with_subgraph_check {
        return Ok(output);
    }

    eprintln!("Checking for invalid sub-graphs");
    // Check for incomplete sub-graphs
    for item in graph.get("mntner").ok_or("can't find mntner category")? {
        if item.extra.deleted.get() {
            continue;
        }

        let mut graph_has_asn = false;

        let mut visited: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        visited.push(item.clone());
        to_visit.push(item.clone());

        while let Some(obj) = to_visit.pop() {
            if obj.extra.deleted.get() {
                continue;
            }
            if obj.category == "aut-num" {
                graph_has_asn = true;
                break;
            }

            link_recurse(&obj, &mut visited, &mut to_visit);
        }
        if !graph_has_asn {
            eprintln!("Warning: Deleting invalid sub-graph for item '{}': {:?}", item.object.filename,
                      visited.iter().map(|x| x.object.filename.clone()).collect::<Vec<_>>());
            for visited in &visited.iter()
                .filter(|x| !x.extra.deleted.get()).collect::<Vec<_>>() {
                visited.extra.deleted.set(true);
                output.push_str(&format!("rm 'data/{}/{}'\n", visited.category, visited.object.filename));
            }
        }
    }

    Ok(output)
}


fn link_recurse(obj: &Rc<LinkedRegistryObject<MetaData>>,
                visited: &mut Vec<Rc<LinkedRegistryObject<MetaData>>>,
                to_visit: &mut Vec<Rc<LinkedRegistryObject<MetaData>>>) {
    for link in obj.forward_links.borrow().iter()
        .chain(obj.back_links.borrow().iter()) {
        let mut found = false;
        for visited in &mut *visited {
            // Do not visit a vertex twice
            if Rc::ptr_eq(&link.upgrade().unwrap(), visited) {
                found = true;
                break;
            }
        }
        if found {
            continue;
        }

        // If not visited already
        visited.push(link.upgrade().unwrap());
        to_visit.push(link.upgrade().unwrap());
    }
}