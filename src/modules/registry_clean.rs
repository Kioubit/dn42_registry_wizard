use std::rc::Rc;
use crate::modules::registry_graph::{create_registry_graph, LinkedRegistryObject};
use crate::modules::util;
use crate::modules::util::BoxResult;


pub fn output(registry_root: String, mnt_file: String) -> BoxResult<String> {
    let weakly_referencing: [&str; 1] = ["as-set"];

    let mut output = String::new();
    let mnt_file = util::read_lines(mnt_file)?.map_while(Result::ok).collect::<Vec<String>>().join("\n");
    let mnt_list = mnt_file.split(",").collect::<Vec<&str>>();

    let graph = create_registry_graph(registry_root.to_owned())?;
    let mntner = graph.get("mntner").ok_or("mntner not found")?;

    // Assuming the registry objects form an undirected graph which is a superset of many disconnected sub-graphs
    // Mark all mntner vertices to delete
    eprintln!("Marking mntners to delete");
    for mnt in mntner {
        let mnt = mnt.clone();
        if mnt_list.contains(&&*mnt.object.filename) {
            mnt.marked.set(true);
        }
    }

    eprintln!("Iterating through every unmarked mntner");
    // For every *unmarked* vertex
    for mnt in mntner {
        if mnt.marked.get() {
            continue;
        }
        // Recursively follow each path while keeping track of visited vertices
        let mut visited: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        visited.push(mnt.clone());
        to_visit.push(mnt.clone());

        while let Some(obj) = to_visit.pop() {
            if weakly_referencing.contains(&obj.category.as_str()) {
                continue;
            }

            // If a *marked* mntner vertex is encountered, unmark it and flag it for manual review
            if obj.marked.get() {
                eprintln!("Manual review: {}", obj.object.filename);
                obj.marked.set(false);
            }

            for link in obj.forward_links.borrow().iter()
                .chain(obj.back_links.borrow().iter()) {
                let mut found = false;
                for visited in &visited {
                    // Do not visit a vertex twice
                    if Rc::ptr_eq(link, visited) {
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
    }


    eprintln!("Iterating through every still marked mntner");
    // For every *still marked* mntner vertex: Recursively delete all vertices
    // Recursively follow each path while keeping track of visited vertices
    for mnt in mntner {
        if !mnt.marked.get() {
            continue;
        }
        let mut visited: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        visited.push(mnt.clone());
        to_visit.push(mnt.clone());

        while let Some(obj) = to_visit.pop() {
            if weakly_referencing.contains(&obj.category.as_str()) {
                continue;
            }
            if obj.deleted.get() {
                continue;
            }
            obj.deleted.set(true);
            output.push_str(&format!("rm 'data/{}/{}'\n", obj.category, obj.object.filename));


            for link in obj.forward_links.borrow().iter()
                .chain(obj.back_links.borrow().iter()) {
                let mut found = false;
                for visited in &visited {
                    // Do not visit a vertex twice
                    if Rc::ptr_eq(link, visited) {
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
    }


    for w in weakly_referencing {
        let w_list: Vec<_> = graph.get(w)
            .ok_or("failed to get weakly referenced category from graph")?
            .iter().collect();
        for w_item in w_list {
            let mut found = false;
            for reference in w_item.back_links.borrow().iter()
                .chain(w_item.forward_links.borrow().iter()) {
                if reference.deleted.get() {
                    continue;
                }
                found = true;
            }
            if !found {
                w_item.deleted.set(true);
                output.push_str(&format!("rm 'data/{}/{}'\n", w_item.category, w_item.object.filename));
                continue;
            }
        }
    }


    // Check for remaining dangling references
    for item in graph.values().flatten() {
        if item.deleted.get() {
            continue;
        }

        let mut has_links = false;
        for link in item.back_links.borrow().iter()
            .chain(item.forward_links.borrow().iter()) {
            if !link.deleted.get() {
                has_links = true;
                continue;
            }
            output.push_str(&format!("sed -i '/{}/d' 'data/{}/{}'\n", link.object.filename, item.category, item.object.filename));
        }

        if !has_links {
            output.push_str(&format!("rm 'data/{}/{}'\n", item.category, item.object.filename));
        }
    }

    Ok(output)
}