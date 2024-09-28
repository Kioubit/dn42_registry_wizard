use std::rc::Rc;
use crate::modules::registry_graph::{create_registry_graph, LinkedRegistryObject};
use crate::modules::util;
use crate::modules::util::BoxResult;

pub fn output(registry_root: String, mnt_file: String) -> BoxResult<String>{
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

    let mut encountered_as_sets : Vec<Rc<LinkedRegistryObject>> = Vec::new();

    eprintln!("Iterating through every unmarked mntner");
    // For every *unmarked* vertex
    for mnt in mntner {
        if mnt.marked.get() {
            continue
        }
        // Recursively follow each path while keeping track of visited vertices
        let mut visited: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        visited.push(mnt.clone());
        to_visit.push(mnt.clone());

        while let Some(obj) = to_visit.pop() {
            if obj.category == "as-set" {
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
            continue
        }
        let mut visited: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        let mut to_visit: Vec<Rc<LinkedRegistryObject>> = Vec::new();
        visited.push(mnt.clone());
        to_visit.push(mnt.clone());

        while let Some(obj) = to_visit.pop() {
            if obj.category == "as-set" {
                if encountered_as_sets.iter().filter(|x| Rc::ptr_eq(*x, &obj)).count() == 0 {
                    encountered_as_sets.push(obj.clone());
                }
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

    
    let deleted_aut_nums : Vec<_> = graph.get("aut-num").ok_or("failed to get aut-num from graph")?
        .iter()
        .filter(|x| x.deleted.get()).collect();
    for as_set in encountered_as_sets {
        let mut found = false;
        for reference in as_set.back_links.borrow().iter()
            .chain(as_set.forward_links.borrow().iter()) {
            if reference.category == "aut-num" {
                continue
            }
            found = true;
        }
        if !found {
            as_set.deleted.set(true);
            output.push_str(&format!("rm 'data/{}/{}'\n", as_set.category, as_set.object.filename));
            continue;
        }
        
        let members = as_set.object.key_value.get("members");
        if members.is_none() {
            continue;
        }
        let members = members.unwrap();
        for member in members.iter() {
            if !deleted_aut_nums.iter()
                .filter(|x| x.object.filename == *member)
                .collect::<Vec<_>>().is_empty() {
                output.push_str(&format!("sed -i '/{}/d' 'data/{}/{}'\n", member, as_set.category, as_set.object.filename));
            }
        }

    }

    Ok(output)
}