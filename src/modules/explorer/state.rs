use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use serde::Serialize;
use crate::modules::object_reader::{OrderedObjectLine, RegistryObject};
use crate::modules::registry_graph::{create_registry_graph, parse_registry_schema, LinkInfoLineNumberOnly, RegistryGraph};
use crate::modules::util::{get_current_unix_time, get_git_commit_hash, BoxResult};


#[derive(Default)]
pub(super) struct AppState {
    pub objects: HashMap<String, Vec<WebRegistryObject>>,
    pub index: HashMap<String, Vec<String>>,
    pub etag: String,
    pub commit_hash: String,
    pub roa4: Option<String>,
    pub roa6: Option<String>,
    pub roa_json: Option<String>,
    pub roa_disabled: bool
}

#[derive(Debug, Serialize)]
pub(super) struct WebRegistryObject {
    pub object: RegistryObject<OrderedObjectLine>,
    pub category: String,
    pub back_links: Vec<String>,
    pub forward_links: Vec<(LinkInfoLineNumberOnly, String)>,
}


pub(super) async fn update_registry_data(registry_root: PathBuf, app_state: Arc<RwLock<AppState>>, with_roa: bool) -> BoxResult<()> {
    let schema = parse_registry_schema(registry_root.as_ref(), false)?;
    let graph: RegistryGraph<(), OrderedObjectLine, LinkInfoLineNumberOnly> = create_registry_graph(registry_root.as_ref(), &schema, true, true)?;
    let commit_hash = get_git_commit_hash(&registry_root).unwrap_or(String::from("N/A"));
    
    let mut graph_web = HashMap::with_capacity(graph.capacity());
    let mut index_map: HashMap<String, Vec<String>> = HashMap::with_capacity(graph.capacity());
    for (c, x) in &graph {
        let mut list = Vec::with_capacity(x.len());
        let mut name_list = Vec::with_capacity(x.len());
        for elem in x {
            let v = WebRegistryObject {
                object: elem.object.clone(),
                category: elem.category.clone(),
                back_links: elem.get_back_links()
                    .map(|x| x.1.category.clone() + "/" + &x.1.object.filename).collect(),
                forward_links: elem.get_forward_links()
                    .map(|(li, obj)| (li, obj.category.clone() + "/" + &obj.object.filename)).collect(),
            };
            name_list.push(elem.object.filename.clone());
            list.push(v);
        }
        index_map.insert(c.clone(), name_list);
        graph_web.insert(c.clone(), list);
    }

    let mut roa4 = None;
    let mut roa6 = None;
    let mut roa_json = None;
    if with_roa {
        roa4 = roa_wizard_lib::generate_bird(&registry_root, false).map_err(|x| {
            eprintln!("Error generating bird roa4: {:?}", x);
        }).map(|x| x.0).ok();
        roa6 = roa_wizard_lib::generate_bird(&registry_root, true).map_err(|x| {
            eprintln!("Error generating bird roa6: {:?}", x);
        }).map(|x| x.0).ok();
        roa_json = roa_wizard_lib::generate_json(&registry_root).map_err(|x| {
            eprintln!("Error generating roa JSON: {:?}", x);
        }).map(|x|x.0).ok();
    }

    let mut app_state_lock = app_state.write().unwrap();
    app_state_lock.objects = graph_web;
    app_state_lock.index = index_map;
    app_state_lock.etag = get_current_unix_time().to_string();
    app_state_lock.commit_hash = commit_hash;
    app_state_lock.roa4 = roa4;
    app_state_lock.roa6 = roa6;
    app_state_lock.roa_json = roa_json;
    Ok(())
}