use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use serde::Serialize;
use crate::modules::object_reader::{OrderedObjectLine, RegistryObject};
use crate::modules::registry_graph::{create_registry_graph, parse_registry_schema, LinkInfoLineNumberOnly, RegistryGraph};
use crate::modules::util::{get_current_unix_time, BoxResult};


#[derive(Default)]
pub(super) struct AppState {
    pub objects: HashMap<String, Vec<WebRegistryObject>>,
    pub index: HashMap<String, Vec<String>>,
    pub etag: String,
}

#[derive(Debug, Serialize)]
pub(super) struct WebRegistryObject {
    pub object: RegistryObject<OrderedObjectLine>,
    pub category: String,
    pub back_links: Vec<String>,
    pub forward_links: Vec<(LinkInfoLineNumberOnly, String)>,
}


pub(super) async fn update_registry_data(registry_root: PathBuf, app_state: Arc<RwLock<AppState>>) -> BoxResult<()> {
    let schema = parse_registry_schema(registry_root.as_ref(), false)?;
    let graph: RegistryGraph<(), OrderedObjectLine, LinkInfoLineNumberOnly> = create_registry_graph(registry_root.as_ref(), &schema, true, true)?;

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


    let mut app_state_lock = app_state.write().unwrap();
    app_state_lock.objects = graph_web;
    app_state_lock.index = index_map;
    app_state_lock.etag = get_current_unix_time().to_string();
    Ok(())
}