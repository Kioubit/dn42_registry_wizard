use crate::modules::object_reader::{
    read_registry_object_kv, registry_objects_to_iter, RegistryObjectIterator, SimpleObjectLine,
};
use crate::modules::util::BoxResult;
use cidr_utils::cidr::IpCidr;
use std::net::IpAddr;
use std::path::Path;
use std::str::FromStr;

pub fn output(registry_root: &Path, search_ip: &str) -> BoxResult<String> {
    let search_ip = IpAddr::from_str(search_ip)?;

    let sub_path = if search_ip.is_ipv4() {
        Path::new("data/inetnum/")
    } else {
        Path::new("data/inet6num/")
    };

    let mut registry_objects: RegistryObjectIterator<SimpleObjectLine> =
        registry_objects_to_iter(registry_root, sub_path)?;
    registry_objects.set_enumerate_only(true);
    let mut length = 0;
    let mut current: Option<String> = None;
    for obj in registry_objects {
        let filename = obj?.filename;
        let obj_cidr = IpCidr::from_str(filename.replace('_', "/").as_str());
        if let Ok(obj_cidr) = obj_cidr {
            if obj_cidr.contains(&search_ip) && obj_cidr.network_length() > length {
                length = obj_cidr.network_length();
                current = Some(filename);
            }
        } else {
            eprint!("Failed to parse object '{}'", filename);
        }
    }

    if let Some(current) = current {
        let target_path = Path::new(registry_root).join(sub_path).join(current);
        let key_value = read_registry_object_kv::<SimpleObjectLine>(target_path.as_path())?;
        Ok(serde_json::to_string(&key_value)?)
    } else {
        Err("Failed to find target".into())
    }
}
