use std::collections::HashMap;
use std::process::Command;
use crate::modules::object_reader::{filter_objects_source, read_registry_objects, RegistryObject};
use crate::modules::util;
use crate::modules::util::BoxResult;

pub fn output(registry_root: String, json_file: String, max_inactive_secs: u64) -> BoxResult<String> {
    let mut output = String::new();

    let json_str = util::read_lines(json_file)?.map_while(Result::ok).collect::<Vec<String>>().join("\n");
    let mut active_asn: HashMap<u32, u64> = serde_json::from_str(json_str.as_str())?;

    let cutoff_time = crate::modules::mrt_activity::get_cutoff_time(max_inactive_secs);
    eprintln!("Cutoff time: {}", cutoff_time);

    active_asn.retain( |_, t| *t >= cutoff_time );

    let mut inactive_mnt_list: Vec<String> = Vec::new();

    let mut aut_nums = read_registry_objects(registry_root.clone(), "data/aut-num/", false)?;
    filter_objects_source(&mut aut_nums, String::from("DN42"));

    let mut inactive_aut_nums: Vec<RegistryObject> = Vec::new();
    let mut active_aut_nums: Vec<RegistryObject> = Vec::new();

    for obj in aut_nums {
        let mnt_list = obj.key_value.get("mnt-by");
        if mnt_list.is_none() {
            active_aut_nums.push(obj);
            continue;
        };
        if mnt_list.unwrap().contains(&String::from("DN42-MNT")) {
            active_aut_nums.push(obj);
            continue;
        }
        let asn_str = obj.filename.strip_prefix("AS");
        if asn_str.is_none() {
            active_aut_nums.push(obj);
            continue;
        };
        let asn_u32 = asn_str.unwrap().parse::<u32>();
        if asn_u32.is_err() {
            active_aut_nums.push(obj);
            continue;
        };
        
        if active_asn.contains_key(&asn_u32.unwrap()) {
            active_aut_nums.push(obj);
        } else {
            inactive_aut_nums.push(obj);
        }
    }



    for aut_num in inactive_aut_nums {
        let asn_path = &format!("data/aut-num/{}", aut_num.filename);
        if get_last_git_activity(&registry_root, asn_path)? >= cutoff_time {
            continue;
        }

        if let Some(mnt_by_list) = aut_num.key_value.get("mnt-by") {
            inactive_mnt_list.append(&mut mnt_by_list.clone());
        } else {
            continue;
        }

        output.push_str(asn_path);
        output.push('\n');
    }
    
    for active_aut_num in active_aut_nums {
        let mnt_list = active_aut_num.key_value.get("mnt-by");
        if mnt_list.is_none() {
            continue;
        }
        for active_mnt in mnt_list.unwrap() {
            if let Some(index) = inactive_mnt_list.iter().position(|val| val == active_mnt) {
                inactive_mnt_list.swap_remove(index);
            }
        }
    }
    
    let mut route_objects_v4 = read_registry_objects(registry_root.clone(), "data/route/", false)?;
    filter_objects_source(&mut route_objects_v4, String::from("DN42"));
    let mut route_objects_v6 = read_registry_objects(registry_root.clone(), "data/route6/", false)?;
    filter_objects_source(&mut route_objects_v6, String::from("DN42"));

    route_objects_v4.retain(|o| !route_object_is_active(o, &active_asn));
    route_objects_v6.retain(|o| !route_object_is_active(o, &active_asn));


    let mut inetnum_objects_v4 = read_registry_objects(registry_root.clone(), "data/inetnum/", false)?;
    filter_objects_source(&mut route_objects_v4, String::from("DN42"));
    let mut inetnum_objects_v6 = read_registry_objects(registry_root.clone(), "data/inet6num/", false)?;
    filter_objects_source(&mut route_objects_v6, String::from("DN42"));

    inetnum_objects_v4.retain(|o| !object_is_active(o, &inactive_mnt_list));
    inetnum_objects_v6.retain(|o| !object_is_active(o, &inactive_mnt_list));


    for object in route_objects_v4 {
        let route_path = &format!("data/route/{}", object.filename);
        //if get_last_git_activity(&registry_root, route_path)? >= cutoff_time {
        //    continue;
        //};
        output.push_str(route_path);
        output.push('\n');
    }

    for object in inetnum_objects_v4 {
        let inetnum_path = &format!("data/inetnum/{}", object.filename);
        //if get_last_git_activity(&registry_root, inetnum_path).unwrap_or(cutoff_time) >= cutoff_time {
        //    continue;
        //};
        output.push_str(inetnum_path);
        output.push('\n');
    }

    for object in route_objects_v6 {
        let route_path = &format!("data/route6/{}", object.filename);
        //if get_last_git_activity(&registry_root, route_path)? >= cutoff_time {
        //    continue;
        //};
        output.push_str(route_path);
        output.push('\n');
    }

    for object in inetnum_objects_v6 {
        let inetnum_path = &format!("data/inet6num/{}", object.filename);
        //if get_last_git_activity(&registry_root, inetnum_path)? >= cutoff_time {
        //    continue;
        //};
        output.push_str(inetnum_path);
        output.push('\n');
    }


    Ok(output)
}

fn get_last_git_activity(registry_root: &str, path :&str) -> BoxResult<u64> {
        let cmd_output = Command::new("git")
            .arg("log")
            .arg("-1")
            .arg("--format=%ct")
            .arg(path)
            .current_dir(registry_root)
            .output()?;
        if !cmd_output.status.success() {
            eprintln!("{:?}",String::from_utf8_lossy(&cmd_output.stderr));
            return Err("git log failed".into());
        }
    let output = String::from_utf8(cmd_output.stdout)?;
    let output_clean = match output.strip_suffix('\n') {
        Some(s) => s,
        None => output.as_str()
    };
    Ok(output_clean.parse::<u64>()?)
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

    found
}

fn object_is_active(object: &RegistryObject, inactive_mnt_list: &Vec<String> ) -> bool {
    let empty_vec: Vec<String> = vec![];

    let mnt_list = object.key_value.get("mnt-by").unwrap_or(&empty_vec);

    let mut found = false;

    for mnt in mnt_list{
        if !inactive_mnt_list.contains(mnt) {
            // If we found at least one active MNT
            found = true;
            break;
        }
    }

    found
}