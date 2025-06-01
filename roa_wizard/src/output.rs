use std::path::Path;
use std::process::Command;
use crate::parse::RouteObject;
use std::time::SystemTime;


pub fn output_bird(objects: Vec<RouteObject>, base_path: &Path) -> String {
    let mut result = format!("# {} {} - Kioubit.dn42\n", crate::PACKAGE_NAME, crate::VERSION);
    result.push_str(&format!("# Created: {}\n", get_sys_time_in_secs()));
    let commit_hash = get_git_commit_hash(base_path);
    if commit_hash.is_some() {
        result.push_str(&format!("# Commit: {}\n", commit_hash.unwrap()));
    }
    for object in objects {
        result.push_str(&object.get_bird_format());
    }
    result
}

pub fn output_json(objects: Vec<RouteObject>) -> String {
    let mut top = json::JsonValue::new_object();
    let mut metadata = json::JsonValue::new_object();

    let mut data = json::JsonValue::new_array();
    let mut count = 0;
    for object in objects {
        for v in object.get_json_objects() {
            data.push(v).expect("Error converting data to JSON");
            count += 1;
        }
    }

    metadata["counts"] = count.into();
    let now = get_sys_time_in_secs();
    metadata["generated"] = now.into();
    metadata["valid"] = (now + 604800).into(); // 7 days

    top["metadata"] = metadata;
    top["roas"] = data;

    top.dump()
}

fn get_sys_time_in_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("SystemTime before UNIX EPOCH").as_secs()
}

fn get_git_commit_hash(path: &Path) -> Option<String> {
    let cmd_output = Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--format=%H")
        .current_dir(path)
        .output().ok()?;
    if !cmd_output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&cmd_output.stdout).to_string())
}