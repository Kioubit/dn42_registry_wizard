use crate::modules::util;
use crate::modules::util::BoxResult;
use bgpkit_parser::models::{AsPath, Asn, AttributeValue, MrtMessage};
use bgpkit_parser::{BgpkitParser, MrtRecord};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{thread, time};

pub fn output(mrt_root: String, max_inactive_secs: u64, output_as_list: bool) -> BoxResult<String> {
    let active_asn = get_active_asn_list(mrt_root, max_inactive_secs)?;
    if output_as_list {
        Ok(active_asn.keys().map(|key| key.to_string()).collect::<Vec<String>>().join(","))
    } else {
        let active_json = serde_json::to_string(&active_asn)?;
        Ok(active_json)
    }
}

pub fn get_cutoff_time(max_inactive_secs: u64) -> u64 {
    if max_inactive_secs == 0 {
        eprintln!("Cutoff time: not set");
        0
    } else {
        let mut time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        time -= time::Duration::new(max_inactive_secs, 0);
        let cutoff_time = time.as_secs();
        eprintln!("Cutoff time: {}", cutoff_time);
        cutoff_time
    }
}

fn get_active_asn_list(mrt_root: String, max_inactive_secs: u64) -> BoxResult<HashMap<u32, u64>> {
    let cutoff_time = get_cutoff_time(max_inactive_secs);

    let paths = util::walk_dir(mrt_root, 10)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(10)
        .build()?;

    let mut result_map: HashMap<u32, u64> = HashMap::new();
    let mut remaining_file_count = paths.len();

    let cancelled = Arc::new(AtomicBool::new(false));

    let (tx, rx) = mpsc::channel();

    thread::scope(|ts| {
        ts.spawn(|| {
            for x in rx {
                remaining_file_count -= 1 ;
                eprintln!("Remaining files: {}", remaining_file_count);
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
                    let cancelled = cancelled.clone();
                    s.spawn(move |_| {
                        if cancelled.load(Ordering::Relaxed) {
                            return
                        }
                        let mut x: HashMap<u32, u64> = HashMap::new();
                        if let Err(err) = analyze_mrt_file(path.to_str().unwrap_or_default(), &mut x, cutoff_time as u32) {
                            eprintln!("Error parsing {:?}: {:?}", path, err);
                            cancelled.store(true, Ordering::Relaxed);
                            return
                        }
                        tx.send(x).unwrap();
                    })
                }
            });
            drop(tx);
        });
    });

    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled due to error".into());
    }

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
            break;
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
        return Err("No records found".into());
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