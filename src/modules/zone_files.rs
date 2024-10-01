use crate::modules::object_reader;
use crate::modules::object_reader::registry_objects_to_iter;
use crate::modules::util::BoxResult;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

static STATIC_ENTRIES: [(&str, &str); 7] = [
    ("20.172.in-addr.arpa", "inetnum/172.20.0.0_16"),
    ("21.172.in-addr.arpa", "inetnum/172.21.0.0_16"),
    ("22.172.in-addr.arpa", "inetnum/172.22.0.0_16"),
    ("23.172.in-addr.arpa", "inetnum/172.23.0.0_16"),
    ("31.172.in-addr.arpa", "inetnum/172.31.0.0_16"),
    ("10.in-addr.arpa", "inetnum/10.0.0.0_8"),
    ("d.f.ip6.arpa", "inet6num/fd00::_8")
];


pub fn output_forward_zones(registry_root: String, auth_servers: Vec<String>) -> BoxResult<String> {
    let mut output = String::new();
    let mut objects = read_tld_objects(registry_root, true)
        .map_err(|e| format!("Error reading objects: {}", e))?;
    objects.sort_by(|a, b| a.tld.cmp(&b.tld));
    output += "recursor:\n";
    output += "  forward_zones:\n";
    for object in objects {
        let current_auth_servers: Vec<String> = if object.n_server_v4.is_empty() && object.n_server_v6.is_empty() {
            auth_servers.iter().map(|s| {
                IpAddr::from_str(s)
                    .map(|parsed| {
                        if parsed.is_ipv6() {
                            String::from('\'') + &*parsed.to_string() + "\'"
                        } else {
                            parsed.to_string()
                        }
                    }).map_err(|e| format!("Could not parse provided authoritative server IP {}", e))
            }).collect::<Result<Vec<String>, String>>()?
        } else {
            object.n_server_v4.into_iter().chain(object.n_server_v6.into_iter().map(|s| "'".to_owned() + &*s + "'")).collect()
        };
        output += format!("  - zone: '{}'\n", object.tld).as_str();
        output += "    forwarders:\n";
        for auth_server in current_auth_servers {
            output += format!("    - {}\n", auth_server).as_str();
        }
    }
    Ok(output)
}

pub fn output_tas(registry_root: String) -> BoxResult<String> {
    let mut output = String::new();
    let objects = read_tld_objects(registry_root, false)
        .map_err(|e| format!("Error reading objects: {}", e))?;
    for object in objects {
        if object.ds_rdata.is_empty() {
            output+= format!("addNTA(\"{}\")\n", object.tld).as_str();
            continue;
        }
        for ta in object.ds_rdata {
            output += format!("addTA('{}',\"{}\")\n", object.tld, ta).as_str();
        }
    };
    Ok(output)
}

#[derive(Debug)]
struct TldObject<> {
    tld: String,
    n_server_v4: Vec<String>,
    n_server_v6: Vec<String>,
    ds_rdata: Vec<String>,
}

#[derive(Debug)]
struct TldObjectBuilder<> {
    tld: Option<String>,
    n_server: Vec<String>,
    ds_rdata: Vec<String>,
    mnt: Option<String>,
}

impl TldObjectBuilder {
    fn new() -> Self {
        Self {
            tld: None,
            ds_rdata: Vec::new(),
            n_server: Vec::new(),
            mnt: None,
        }
    }
    fn build(self, is_reverse: bool, show_nameserver_note: bool) -> BoxResult<TldObject> {
        if self.tld.is_none() || self.mnt.is_none() || self.n_server.is_empty() {
            Err("missing fields")?
        }
        if self.mnt.unwrap() != "DN42-MNT" {
            Err("mnt is not dn42")?
        }
        if !is_reverse && self.tld.as_ref().unwrap().contains('.') {
            Err("not a tld")?
        }

        let mut n_servers_v4 = Vec::new();
        let mut n_servers_v6 = Vec::new();
        for server in self.n_server {
            if server.ends_with(".dn42") {
                if server.ends_with(".ipv4.registry-sync.dn42") {
                    let reverse_notation = server.strip_suffix(".ipv4.registry-sync.dn42").unwrap_or_default();
                    n_servers_v4.push(parse_reverse_ip_notation(reverse_notation, false));
                } else if server.ends_with(".ipv6.registry-sync.dn42") {
                    let reverse_notation = server.strip_suffix(".ipv6.registry-sync.dn42").unwrap_or_default();
                    n_servers_v6.push(parse_reverse_ip_notation(reverse_notation, true));
                } else if show_nameserver_note {
                    eprintln!("Encountered nameserver that needs to be resolved: '{}' for '{}' (Will use provided authoritative servers)", server, self.tld.as_ref().unwrap_or(&"N/A".to_string()));
                }
            } else if let Some(split_server) = server.split_once(' ') {
                let parsed_server_ip = IpAddr::from_str(split_server.1);
                if parsed_server_ip.is_err() {
                    eprintln!("Failed to parse nameserver IP: {} for {:?}", server, self.tld);
                    continue;
                }
                if parsed_server_ip.as_ref().unwrap().is_ipv6() {
                    n_servers_v6.push(parsed_server_ip.unwrap().to_string())
                } else {
                    n_servers_v4.push(parsed_server_ip.unwrap().to_string())
                }
            } else {
                eprintln!("Encountered unknown nameserver format: {} for {:?}", server, self.tld);
            }
        }

        let obj = TldObject {
            tld: self.tld.unwrap(),
            ds_rdata: self.ds_rdata,
            n_server_v4: n_servers_v4,
            n_server_v6: n_servers_v6,
        };
        Ok(obj)
    }
}

fn parse_reverse_ip_notation(n: &str, is_v6: bool) -> String {
    let fields = n.split('.').rev();
    let mut result: String = String::new();
    if is_v6 {
        for (i, field) in fields.into_iter().enumerate() {
            result.push_str(field);
            if (i + 1) % 4 == 0 {
                result.push(':');
            }
        }
        result.pop();
    } else {
        for field in fields {
            result.push_str(field);
            result.push('.');
        }
        result.pop();
    }
    result
}


fn read_tld_objects(registry_root: String, show_nameserver_note: bool) -> BoxResult<Vec<TldObject>> {
    let mut tld_objects: Vec<TldObject> = Vec::new();
    let mut registry_objects = registry_objects_to_iter(registry_root.clone(), "data/dns")?;
    registry_objects.add_filename_filter(".");
    registry_objects.add_exclusive_fields(vec![
        String::from("domain"),
        String::from("mnt-by"),
        String::from("ds-rdata"),
        String::from("nserver"),
    ]);
    for obj in registry_objects {
        let obj = obj?;
        let mut tld_builder = TldObjectBuilder::new();
        tld_builder.tld = obj.key_value.get("domain").and_then(|x| x.first().cloned());
        tld_builder.mnt = obj.key_value.get("mnt-by").and_then(|x| x.first().cloned());
        if let Some(v) = obj.key_value.get("ds-rdata") {
            tld_builder.ds_rdata.extend(v.iter().cloned());
        }
        if let Some(v) = obj.key_value.get("nserver") {
            tld_builder.n_server.extend(v.iter().cloned());
        }
        tld_objects.push(tld_builder.build(false, show_nameserver_note)?)
    }

    for entry in STATIC_ENTRIES {
        tld_objects.push(get_static_entry(&registry_root, entry, show_nameserver_note)?);
    }

    Ok(tld_objects)
}


fn get_static_entry(registry_root: &str, entry: (&str, &str), show_nameserver_note: bool) -> BoxResult<TldObject> {
    let file = registry_root.to_owned() + "/data/" + entry.1;
    let registry_kv = object_reader::read_registry_object_kv(PathBuf::from(file))?;

    let mut tld_builder = TldObjectBuilder::new();
    tld_builder.tld = Some(entry.0.to_owned());
    tld_builder.mnt = registry_kv.get("mnt-by").and_then(|x| x.first().cloned());
    if let Some(v) = registry_kv.get("ds-rdata") {
        tld_builder.ds_rdata.extend(v.iter().cloned());
    }
    tld_builder.n_server.push("delegation-servers.dn42".to_string());

    let result = tld_builder.build(true, show_nameserver_note)?;
    Ok(result)
}
