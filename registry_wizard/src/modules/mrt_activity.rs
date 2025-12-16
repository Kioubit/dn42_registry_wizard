use crate::modules::util;
use crate::modules::util::BoxResult;
use bgpkit_parser::{BgpkitParser, MrtUpdate};
use rayon::prelude::*;
use std::collections::HashMap;
use std::cmp;

pub fn output(mrt_root: String, cutoff_time: u64, output_as_list: bool) -> BoxResult<String> {
    let active_asn = get_active_asn_list(mrt_root, cutoff_time)?;
    if output_as_list {
        Ok(active_asn.keys().map(|key| key.to_string()).collect::<Vec<String>>().join(","))
    } else {
        Ok(serde_json::to_string(&active_asn)?)
    }
}

fn get_active_asn_list(mrt_root: String, cutoff_time: u64) -> BoxResult<HashMap<u32, u64>> {
    let paths = util::walk_dir(mrt_root, 10)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(10)
        .build()?;

    let result_map = pool.install(|| {
        paths.par_iter().map(|path| -> BoxResult<_> {
            let mut local_map = HashMap::new();
            analyze_mrt_file(
                path.to_str().unwrap_or_default(),
                &mut local_map,
                cutoff_time,
            )?;
            Ok(local_map)
        }).try_reduce(HashMap::new, |mut acc, map| {
            for (t_asn, t_time) in map {
                acc.entry(t_asn)
                    .and_modify(|existing| *existing = (*existing).max(t_time))
                    .or_insert(t_time);
            }
            Ok(acc)
        })
    })?;

    Ok(result_map)
}

fn analyze_mrt_file(path: &str, acc_map: &mut HashMap<u32, u64>, cutoff_time: u64) -> BoxResult<()> {
    eprintln!("Parsing {}", path);
    let parser = BgpkitParser::new(path)?;
    let mut had_record = false;

    let mut process_entry = |attributes: &bgpkit_parser::models::Attributes, timestamp: u64| {
        if let Some(path) = attributes.as_path() {
            for origin in path.iter_origins() {
                acc_map.entry(origin.to_u32())
                    .and_modify(|t| *t = cmp::max(*t, timestamp))
                    .or_insert(timestamp);
            }
        }
    };


    for update in parser.into_update_iter() {
        had_record = true;
        let timestamp = update.timestamp() as u64;
        if timestamp < cutoff_time {
            // Each RIB dump file only contains records from the same timestamp
            break;
        }
        match update {
            MrtUpdate::TableDumpMessage(m) => process_entry(&m.attributes, timestamp),
            MrtUpdate::Bgp4MpUpdate(u) => process_entry(&u.message.attributes, timestamp),
            MrtUpdate::TableDumpV2Entry(e) => {
                for entry in e.rib_entries {
                    process_entry(&entry.attributes,timestamp);
                }
            }
        }
    }

    if !had_record {
        return Err("No records found".into());
    }

    eprintln!("Completed {}", path);
    Ok(())
}