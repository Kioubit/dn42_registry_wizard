use std::fs::{read_dir};
use std::net::IpAddr;
use std::str::FromStr;
use crate::modules::util;

static STATIC_ENTRIES: [(&str,&str);7] = [
    ("20.172.in-addr.arpa","inetnum/172.20.0.0_16"),
    ("21.172.in-addr.arpa","inetnum/172.21.0.0_16"),
    ("22.172.in-addr.arpa","inetnum/172.22.0.0_16"),
    ("23.172.in-addr.arpa","inetnum/172.23.0.0_16"),
    ("31.172.in-addr.arpa","inetnum/172.31.0.0_16"),
    ("10.in-addr.arpa","inetnum/10.0.0.0_8"),
    ("d.f.ip6.arpa","inet6num/fd00::_8")
];


pub fn output_forward_zones_legacy(registry_root: String, auth_servers: Vec<String>) {
    let mut objects = read_tld_objects(registry_root).expect("Error reading objects");
    objects.sort_by(|a,b | a.tld.cmp(&b.tld));
    let mut first = true;
    for object in objects {
        let current_auth_servers: Vec<String> = if object.n_server_v4.is_empty() && object.n_server_v6.is_empty() {
            auth_servers.iter().map(|s| {
                let parsed = IpAddr::from_str(s).expect("Could not parse provided authoritative server IP");
                parsed.to_string()
            }).collect()
        } else {
            object.n_server_v4.into_iter().chain(object.n_server_v6.into_iter()).collect()
        };
        for auth_server in current_auth_servers {
            if first {
                println!("forward-zones={}={}", object.tld, auth_server);
                first = false;
            }
            println!("forward-zones+={}={}", object.tld, auth_server);
        }
    }
}


pub fn output_forward_zones(registry_root: String, auth_servers: Vec<String>) {
    let mut objects = read_tld_objects(registry_root).expect("Error reading objects");
    objects.sort_by(|a,b | a.tld.cmp(&b.tld));
    println!("recursor:");
    println!("  forward_zones:");
    for object in objects {
        let current_auth_servers: Vec<String> = if object.n_server_v4.is_empty() && object.n_server_v6.is_empty() {
            auth_servers.iter().map(|s| {
                let parsed = IpAddr::from_str(s).expect("Could not parse provided authoritative server IP");
                if parsed.is_ipv6() {
                    String::from('\'') + &*parsed.to_string() + "\'"
                } else {
                    parsed.to_string()
                }
            }).collect()
        } else {
            object.n_server_v4.into_iter().chain(object.n_server_v6.into_iter().map(|s| "'".to_owned() + &*s +  "'")).collect()
        };
        println!("  - zone: '{}'", object.tld);
        println!("    forwarders:");
        for auth_server in current_auth_servers {
            println!("    - {}", auth_server);
        }
    }
}

pub fn output_tas(registry_root: String) {
    let objects = read_tld_objects(registry_root).expect("Error reading objects");
    for object in objects {
        if object.ds_rdata.is_empty() {
            println!("addNTA(\"{}\")", object.tld);
            continue;
        }
        for ta in object.ds_rdata {
            println!("addTA('{}',\"{}\")", object.tld, ta);
        }
    }
}

fn get_static_entry(registry_root: &String, entry : (&str, &str)) -> util::BoxResult<TldObject> {
    let lines = util::read_lines(registry_root.to_owned() + "/data/" + entry.1)?;
    let mut object = TldObjectBuilder::new();
    object.tld = Some(entry.0.to_owned());
    for line in lines {
        if let Some(result) = line?.split_once(':') {
            match result.0.trim_end() {
                "ds-rdata" => { object.ds_rdata.push(result.1.trim().to_owned()) }
                "mnt-by" => { object.mnt = Some(result.1.trim().to_owned()) }
                &_ => {}
            }
        }
    }
    object.n_server.push("delegation-servers.dn42".to_string());
    let result = object.build(true)?;
    Ok(result)
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
    fn build(self, is_reverse : bool) -> util::BoxResult<TldObject> {
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
                    n_servers_v4.push(parse_reverse_ip_notation(reverse_notation,false));
                } else if server.ends_with(".ipv6.registry-sync.dn42") {
                    let reverse_notation = server.strip_suffix(".ipv6.registry-sync.dn42").unwrap_or_default();
                    n_servers_v6.push(parse_reverse_ip_notation(reverse_notation, true));
                } else {
                    eprintln!("Encountered nameserver that needs to be resolved: '{}' for '{}' (Will use provided authoritative servers)", server, self.tld.as_ref().unwrap_or(&"N/A".to_string()));
                }
            } else if let Some(split_server) = server.split_once(' ') {
                let parsed_server_ip  = IpAddr::from_str(split_server.1);
                if parsed_server_ip.is_err() {
                    eprintln!("Failed to parse nameserver IP: {} for {:?}", server, self.tld);
                    continue
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

fn parse_reverse_ip_notation(n : &str, is_v6: bool) -> String{
    let fields = n.split('.').rev();
    let mut result: String = String::new();
    if is_v6 {
        for (i,field) in fields.into_iter().enumerate() {
            result.push_str(field);
            if (i+1)%4 == 0 {
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


fn read_tld_objects(registry_root: String) -> util::BoxResult<Vec<TldObject>> {
    let dns_path = registry_root.to_owned() + "data/dns/";
    let mut objects: Vec<TldObject> = Vec::new();
    let dir = read_dir(dns_path)?;
    for file_result in dir {
        let file = file_result?.path();
        let filename = file.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned();
        if filename.contains('.') {
            continue;
        }
        let mut object = TldObjectBuilder::new();
        let lines = util::read_lines(&file)?;
        for line in lines {
            if let Some(result) = line?.split_once(':') {
                match result.0.trim_end() {
                    "domain" => { object.tld = Some(result.1.trim().to_owned()) }
                    "ds-rdata" => { object.ds_rdata.push(result.1.trim().to_owned()) }
                    "nserver" => { object.n_server.push(result.1.trim().to_owned()) }
                    "mnt-by" => { object.mnt = Some(result.1.trim().to_owned()) }
                    &_ => {}
                }
            }
        }
        objects.push(object.build(false)?);
    }

    for entry  in STATIC_ENTRIES {
        objects.push(get_static_entry(&registry_root,entry)?);
    }

    Ok(objects)
}


