use crate::modules::object_reader::{filter_objects_source, read_registry_objects, RegistryObject};
use crate::modules::util;
use crate::modules::util::BoxResult;
use std::collections::HashMap;
use std::process::Command;

pub fn output(registry_root: String, json_file: String, max_inactive_secs: u64) -> BoxResult<String> {
    let json_str = util::read_lines(json_file)?.map_while(Result::ok).collect::<Vec<String>>().join("\n");
    let mut active_asn: HashMap<u32, u64> = serde_json::from_str(json_str.as_str())?;

    let cutoff_time = crate::modules::mrt_activity::get_cutoff_time(max_inactive_secs);
    eprintln!("Cutoff time: {}", cutoff_time);

    active_asn.retain(|_, t| *t >= cutoff_time);

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


    inactive_mnt_list.sort_unstable();
    inactive_mnt_list.dedup();

    let output = inactive_mnt_list.join(",");
    Ok(output)
}

fn get_last_git_activity(registry_root: &str, path: &str) -> BoxResult<u64> {
    let cmd_output = Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%ct")
        .arg(path)
        .current_dir(registry_root)
        .output()?;
    if !cmd_output.status.success() {
        eprintln!("{:?}", String::from_utf8_lossy(&cmd_output.stderr));
        return Err("git log failed".into());
    }
    let output = String::from_utf8(cmd_output.stdout)?;
    let output_clean = match output.strip_suffix('\n') {
        Some(s) => s,
        None => output.as_str()
    };
    Ok(output_clean.parse::<u64>()?)
}