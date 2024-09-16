use crate::modules::object_reader::{filter_objects_source, read_registry_objects, RegistryObject};
use crate::modules::util;
use crate::modules::util::BoxResult;
use bgpkit_parser::models::{AsPath, Asn, AttributeValue, MrtMessage};
use bgpkit_parser::{BgpkitParser, MrtRecord};
use std::collections::HashMap;
use std::ops::Sub;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{thread, time};

pub fn output(registry_root: String, mrt_root: String, max_inactive_secs: u64, with_registry: bool) -> BoxResult<String> {
    let mut output = String::new();

    let active_asn = get_active_asn_list(mrt_root, max_inactive_secs)?;
    let active_json = serde_json::to_string(&active_asn)?;
    output.push_str(&active_json);
    output.push('\n');

    if !with_registry {
        return Ok(output);
    }

    let mut route_objects_v4 = read_registry_objects(registry_root.clone(), "data/route/", false)?;
    filter_objects_source(&mut route_objects_v4, "DN42".to_string());
    let mut route_objects_v6 = read_registry_objects(registry_root, "data/route6/", false)?;
    filter_objects_source(&mut route_objects_v6, "DN42".to_string());

    route_objects_v4.retain(|o| route_object_is_active(o, &active_asn));
    route_objects_v6.retain(|o| route_object_is_active(o, &active_asn));

    for object in route_objects_v4 {
        output.push_str(&format!("data/route/{}\n", object.filename));
        output.push_str(&format!("data/inetnum/{}\n", object.filename));
    }

    for object in route_objects_v6 {
        output.push_str(&format!("data/route6/{}\n", object.filename));
        output.push_str(&format!("data/inet6num/{}\n", object.filename));
    }

    Ok(output)
}

fn route_object_is_active(route_object: &RegistryObject, active_asn: &HashMap<u32, u64>) -> bool {
    let empty_vec: Vec<String> = vec![];

    let origin_asn_vec = route_object.key_value.get("origin").unwrap_or(&empty_vec);
    let origin_asn_vec_u32: Vec<u32> = origin_asn_vec.iter().filter_map(|x|
        x.strip_prefix("AS")?.parse::<u32>().ok()
    ).collect();
    if origin_asn_vec_u32.is_empty() {
        return false;
    }

    let mut found = false;

    for origin_asn in &origin_asn_vec_u32 {
        if active_asn.contains_key(origin_asn) {
            // If we found at least one active origin ASN
            found = true;
            break;
        }
    }
    if !found {
        return false;
    }
    true
}

fn get_active_asn_list(mrt_root: String, max_inactive_secs: u64) -> BoxResult<HashMap<u32, u64>> {
    let cutoff_time = if max_inactive_secs == 0 {
        0
    } else {
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        time.sub(time::Duration::new(max_inactive_secs, 0)).as_secs()
    };

    eprintln!("Cutoff time: {}", cutoff_time);

    let paths = util::walk_dir(mrt_root, 10)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(10)
        .build()?;

    let mut result_map: HashMap<u32, u64> = HashMap::new();

    let (tx, rx) = mpsc::channel();

    thread::scope(|ts| {
        ts.spawn(|| {
            for x in rx {
                for (t_asn, t_time) in x {
                    if let Some(global_data) = result_map.get_mut(&t_asn) {
                        if t_time > *global_data {
                            *global_data = t_time;
                        }
                    } else {
                        result_map.insert(t_asn, t_time);
                    }
                }
            }
        });


        pool.install(|| {
            rayon::scope(|s| {
                for path in paths {
                    let tx = tx.clone();
                    s.spawn(move |_| {
                        let mut x: HashMap<u32, u64> = HashMap::new();
                        if let Err(err) = analyze_mrt_file(path.to_str().unwrap_or_default(), &mut x, cutoff_time as u32) {
                            panic!("Error parsing {:?}: {:?}", path, err);
                        }
                        tx.send(x).unwrap();
                    })
                }
            });
            drop(tx);
        });
    });


    Ok(result_map)
}

fn analyze_mrt_file(path: &str, x: &mut HashMap<u32, u64>, cutoff_time: u32) -> BoxResult<()> {
    eprintln!("Parsing {}", path);
    let parser = BgpkitParser::new(path)?;
    let mut had_record = false;
    for record in parser.into_record_iter() {
        had_record = true;
        let timestamp = record.common_header.timestamp;
        if timestamp < cutoff_time && cutoff_time != 0 {
            // Each RIB dump file only contains records from the same timestamp
            break
        }
        let asn_list = record_to_origin_asn_list(record);
        for asn in asn_list {
            if let Some(last_seen) = x.get_mut(&asn) {
                if *last_seen < timestamp as u64 {
                    *last_seen = timestamp as u64;
                }
            } else {
                x.insert(asn, timestamp as u64);
            }
        }
    }
    if !had_record {
        panic!("No records found in MRT file: {}", path)
    }
    eprintln!("Completed {}", path);
    Ok(())
}

fn record_to_origin_asn_list(record: MrtRecord) -> Vec<u32> {
    let mut origin_asn_list: Vec<Asn> = Vec::new();
    match record.message {
        MrtMessage::TableDumpMessage(msg) => {
            msg.attributes.as_path();
            for attribute in msg.attributes {
                if let AttributeValue::AsPath { path, is_as4: false } = attribute {
                    origin_asn_list.extend(path.iter_origins());
                    break;
                }
            }
        }
        MrtMessage::TableDumpV2Message(bgpkit_parser::models::TableDumpV2Message::RibAfi(t)) => {
            for entry in t.rib_entries {
                let mut as_path: Option<AsPath> = None;
                let mut as4_path: Option<AsPath> = None;
                for attribute in entry.attributes {
                    match attribute {
                        AttributeValue::AsPath { path, is_as4: false } => {
                            as_path = Some(path);
                        }
                        AttributeValue::AsPath { path, is_as4: true } => {
                            as4_path = Some(path);
                        }
                        _ => {}
                    }
                }
                let path = match (as_path, as4_path) {
                    (None, None) => None,
                    (Some(v), None) => Some(v),
                    (None, Some(v)) => Some(v),
                    (Some(v1), Some(v2)) => {
                        Some(AsPath::merge_aspath_as4path(&v1, &v2))
                    }
                };
                if let Some(list) = path
                    .as_ref()
                    .map(|as_path| as_path.iter_origins().collect::<Vec<_>>()) {
                    origin_asn_list.extend(list);
                }
            }
        }
        MrtMessage::TableDumpV2Message(bgpkit_parser::models::TableDumpV2Message::PeerIndexTable(_)) => {}
        _ => {
            panic!("Unsupported MRT subtype")
        }
    }
    let asn_vec: Vec<u32> = origin_asn_list.into_iter()
        .map(|x| x.to_u32()).collect();
    asn_vec
}