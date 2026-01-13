use crate::RoaData;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;
use json::JsonValue;
use crate::parse::RouteObject;

impl RouteObject {
    pub fn get_bird_format(&self) -> String {
        let mut result = String::new();
        let prefix = self.get_prefix_string();
        let max_length = self.max_length.unwrap();
        for origin in &self.origins {
            result.push_str(&format!("route {prefix} max {max_length} as {origin};\n", prefix = prefix,
                                     max_length = max_length, origin = origin));
        }
        result
    }
    pub fn get_json_objects(&self) -> Vec<JsonValue> {
        let mut result: Vec<JsonValue> = Vec::new();
        for origin in &self.origins {
            let mut data = JsonValue::new_object();
            data["prefix"] = self.get_prefix_string().into();
            data["maxLength"] = self.max_length.unwrap().into();
            data["asn"] = origin.to_owned().into();
            result.push(data);
        }
        result
    }

    fn get_prefix_string(&self) -> String {
        if self.prefix.is_host_address() {
            return if self.prefix.is_ipv4() {
                self.prefix.to_string() + "/32"
            } else {
                self.prefix.to_string() + "/128"
            };
        }
        self.prefix.to_string()
    }
}

impl RoaData {
    pub fn output_bird(&self, base_path: impl AsRef<Path>) -> String {
        let mut result = format!("# {} {} - Kioubit.dn42\n", crate::PACKAGE_NAME, crate::VERSION);
        result.push_str(&format!("# Created: {}\n", get_sys_time_in_secs()));
        if let Some(commit_hash) = get_git_commit_hash(base_path.as_ref()) {
            result.push_str(&format!("# Commit: {}\n", commit_hash));
        }
        for object in &self.0 {
            result.push_str(object.get_bird_format().as_str());
        }
        result
    }
    pub fn output_json(self) -> String {
        let mut top = json::JsonValue::new_object();
        let mut metadata = json::JsonValue::new_object();

        let mut data = json::JsonValue::new_array();
        let mut count = 0;
        for object in self.0 {
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

        top.dump() + "\n"
    }
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