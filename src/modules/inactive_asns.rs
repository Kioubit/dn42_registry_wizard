use std::path::{Path, PathBuf};
use crate::modules::object_reader::{registry_objects_to_iter, RegistryObjectIterator, SimpleObjectLine};
use crate::modules::util::{get_item_list, get_last_git_activity, BoxResult, EitherOr};

pub fn output(registry_root: &Path, data_input: EitherOr<String, String>, cutoff_time: Option<u64>) -> BoxResult<String> {
    let raw_list = get_item_list(data_input)?;
    let ok = raw_list.chars().all(|c|c == ',' || char::is_numeric(c) || char::is_whitespace(c));
    if !ok {
        return Err("ASN list contains invalid characters".into());
    }
    let active_asn: Vec<String> = raw_list.split(",").map(String::from).collect();
    eprintln!("Active ASN count: {}", active_asn.len());
    let active_asn = active_asn.into_iter()
        .map(|x| format!("AS{}", x.trim())).collect::<Vec<String>>();
    let mut registry_iter : RegistryObjectIterator<SimpleObjectLine> = registry_objects_to_iter(registry_root, Path::new("data/aut-num"))?;
    registry_iter.set_enumerate_only(true);
    
    let mut skipped_count: usize = 0;
    let mut inactive_asn = Vec::new();
    for item in registry_iter {
        let item = item?;
        if active_asn.contains(&item.filename) {
            skipped_count += 1;
            continue;
        }
        if let Some(cutoff_time) = cutoff_time {
            let asn_path = PathBuf::from("data/aut-num/").join(&item.filename);
            let last_activity = get_last_git_activity(registry_root, &asn_path)
                .map_err(|e| format!("Error getting last git activity for {}: {}", asn_path.display(), e))?;
            if last_activity >= cutoff_time {
                skipped_count += 1;
                continue;
            }
        }
        inactive_asn.push(item.filename.strip_prefix("AS").ok_or(format!("ASN file {} does not start with 'AS'", item.filename))?.to_string());
    }
    eprintln!("Found {} active ASNs", skipped_count);
    eprintln!("Found {} inactive ASNs", inactive_asn.len());
    Ok(inactive_asn.join(","))
}