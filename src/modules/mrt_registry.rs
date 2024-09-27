use std::collections::HashMap;
use crate::modules::object_reader::{filter_objects_source, read_registry_objects, RegistryObject};
use crate::modules::util;
use crate::modules::util::BoxResult;

pub fn output(registry_root: String, json_file: String, max_inactive_secs: u64) -> BoxResult<String> {
    let mut output = String::new();

    let json_str = util::read_lines(json_file)?.flat_map(Result::ok).collect::<Vec<String>>().join("\n");
    let mut active_asn: HashMap<u32, u64> = serde_json::from_str(json_str.as_str())?;

    let cutoff_time = crate::modules::mrt_activity::get_cutoff_time(max_inactive_secs);
    eprintln!("Cutoff time: {}", cutoff_time);

    active_asn.retain( |_, t| *t >= cutoff_time );

    let mut route_objects_v4 = read_registry_objects(registry_root.clone(), "data/route/", false)?;
    filter_objects_source(&mut route_objects_v4, String::from("DN42"));
    let mut route_objects_v6 = read_registry_objects(registry_root.clone(), "data/route6/", false)?;
    filter_objects_source(&mut route_objects_v6, String::from("DN42"));

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

    let mut aut_nums = read_registry_objects(registry_root, "data/aut-num/", false)?;
    filter_objects_source(&mut aut_nums, String::from("DN42"));
    aut_nums.retain(|obj| {
        let mnt_list = obj.key_value.get("mnt-by");
        if mnt_list.is_none() {return false};
        if mnt_list.unwrap().contains(&String::from("DN42-MNT")) {return false;}
        let asn_str = obj.filename.strip_prefix("AS");
        if asn_str.is_none() {return false};
        let asn_u32 = asn_str.unwrap().parse::<u32>();
        if asn_u32.is_err() {return false};
        // Retain only when the ASN are not present in the active asn vec
        !active_asn.contains_key(&asn_u32.unwrap())
    });

    for aut_num in aut_nums {
        output.push_str(&format!("data/aut-num/{}\n", aut_num.filename));
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