use crate::modules::registry_graph::{create_registry_graph, link_visit, parse_registry_schema, ExtraDataTrait, LinkedRegistryObject, WEAKLY_REFERENCING};
use crate::modules::util::{get_item_list, BoxResult, EitherOr};
use serde::Serialize;
use std::cell::Cell;
use std::rc::Rc;


#[derive(Debug, Serialize, Default)]
struct MetaData {
    marked: Cell<bool>,
    deleted: Cell<bool>,
}
impl ExtraDataTrait for MetaData {}

pub enum RemovalCategory {
    Mnt,
    Asn
}

impl RemovalCategory {
    fn as_str(&self) -> &str {
        match self {
            RemovalCategory::Mnt => "mntner",
            RemovalCategory::Asn => "aut-num"
        }
    }
}

pub fn output(registry_root: String, data_input: EitherOr<String, String>,
              removal_category: RemovalCategory,
              with_subgraph_check: bool) -> BoxResult<String> {
    if !with_subgraph_check {
        eprintln!("Warning: Subgraph check has been disabled")
    }

    let raw_list = get_item_list(data_input)?;

    let mut output = String::new();
    let registry_schema = parse_registry_schema(registry_root.to_owned())?;
    let graph = create_registry_graph::<MetaData>(registry_root.to_owned(), &registry_schema)?;

    let removal_list: Vec<String>;
    let affected_graph;
    match removal_category {
        RemovalCategory::Mnt => {
            removal_list = raw_list.split(",").map(String::from).collect();
            affected_graph = graph.get("mntner").ok_or("mntner graph not found")?;
        }
        RemovalCategory::Asn => {
            let ok = raw_list.chars().all(|c|c == ',' || char::is_numeric(c) || char::is_whitespace(c));
            if !ok {
                return Err("ASN list contains invalid characters".into());
            }
            affected_graph = graph.get("aut-num").ok_or("aut-num graph not found")?;
            removal_list = raw_list.split(",").map(String::from)
                .map(|x| format!("AS{}", x.trim())).collect();
        }
    }

    let only_one_removal_item = matches!(removal_list.len(), 1);
    eprintln!("Provided list contains {} object(s)", removal_list.len());

    // Assuming the registry objects form an undirected graph which is a superset of many disconnected sub-graphs
    // Mark all mntner/aut-num vertices to delete
    eprintln!("Analyzing dependency graph (1/6)");
    for t in affected_graph {
        let t = t.clone();
        if removal_list.contains(&t.object.filename) {
            t.extra.marked.set(true);
        }
    }

    // Ensure DN42-MNT is not marked
    graph.get("mntner").ok_or("mntner graph not found")?.iter()
        .find(|x| x.object.filename == "DN42-MNT")
        .ok_or("DN42-MNT not found")?.extra.marked.set(false);

    eprintln!("Analyzing dependency graph (2/6)");
    // For every *marked* vertex
    for t in affected_graph {
        if !t.extra.marked.get() {
            continue;
        }
        // Recursively follow each path while keeping track of visited vertices
        let mut visited: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        visited.push(t.clone());
        to_visit.push(t.clone());

        while let Some(obj) = to_visit.pop() {
            if WEAKLY_REFERENCING.contains(&obj.category.as_str()) {
                continue;
            }
            if &obj.category == "aut-num" && obj.object.filename == "AS0" {
                // Special case
                continue;
            }

            // If an *unmarked* mntner/aut-num vertex is encountered, unmark self and flag for manual review
            let empty_vec : Vec<String> = Vec::with_capacity(0);
            if !obj.extra.marked.get() && obj.category == removal_category.as_str() {
                t.extra.marked.set(false);
                let t_mnt = t.object.key_value.get("mnt-by").unwrap_or(&empty_vec);
                if !t_mnt.contains(&String::from("DN42-MNT")) || only_one_removal_item {
                    eprintln!("Manual review: {} - {:?} (First conflict with active object: {} - {:?})",
                              t.object.filename, t_mnt,
                              obj.object.filename, obj.object.key_value.get("mnt-by").unwrap_or(&empty_vec)
                    );
                }
                if only_one_removal_item && !with_subgraph_check {
                    return Err("Manual review needed".into());
                }
                break;
            }

            link_visit(&obj, &mut visited, &mut to_visit);
        }
    }


    eprintln!("Analyzing dependency graph (3/6)");
    // For every *still marked* mntner/aut-num vertex: Recursively delete all vertices
    // Recursively follow each path while keeping track of visited vertices
    for t in affected_graph {
        if !t.extra.marked.get() {
            continue;
        }
        let mut visited: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject<MetaData>>> = Vec::new();
        visited.push(t.clone());
        to_visit.push(t.clone());

        while let Some(obj) = to_visit.pop() {
            if WEAKLY_REFERENCING.contains(&obj.category.as_str()) {
                continue;
            }
            if &obj.category == "aut-num" && obj.object.filename == "AS0" {
                // Special case
                continue;
            }
            if obj.extra.deleted.get() {
                continue;
            }
            obj.extra.deleted.set(true);
            output.push_str(&format!("rm 'data/{}/{}'\n", obj.category, obj.object.filename));

            link_visit(&obj, &mut visited, &mut to_visit);
        }
    }

    eprintln!("Analyzing dependency graph (4/6)");
    // Check if weakly referenced objects have dangling references
    for w in WEAKLY_REFERENCING {
        let empty_vec = vec![];
        let w_list: Vec<_> = graph.get(w)
            .unwrap_or(&empty_vec)
            .iter().collect();
        for w_item in w_list {
            let mut found = false;
            for reference in w_item.get_back_links()
                .chain(w_item.get_forward_links()) {
                if reference.extra.deleted.get() {
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
        for link in item.get_back_links()
            .chain(item.get_forward_links()) {
            if !link.extra.deleted.get() {
                has_links = true;
                continue;
            }
            output.push_str(&format!("sed -i '/{}/d' 'data/{}/{}'\n", link.object.filename, item.category, item.object.filename));
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
            if !item.get_forward_links()
                .filter(|x| !x.extra.deleted.get())
                .any(|x| x.category == *required_category) {
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
            
            if let Some(m) =  obj.object.key_value.get("mnt-by") {
                if m.iter().any(|m| m == "DN42-MNT") {
                    // Skip sub-graphs containing DN42-MNT
                    graph_has_asn = true;
                    break;
                }
            }

            link_visit(&obj, &mut visited, &mut to_visit);
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