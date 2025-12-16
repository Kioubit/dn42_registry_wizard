use crate::{BoxResult, RouteObjectsWithWarnings};
use cidr_utils::cidr::IpCidr;
use json::JsonValue;
use std::fs::{read_dir, File};
use std::io;
use std::io::BufRead;
use std::path::Path;
use std::str::FromStr;


#[derive(Debug)]
pub struct Filter {
    priority: u32,
    allow: bool,
    prefix: IpCidr,
    min_len: u8,
    max_len: u8,
}

impl Filter {
    fn from_tokens(tokens: &[&str]) -> BoxResult<Self> {
        if tokens.len() < 5 {
            return Err("Insufficient columns".into());
        }
        Ok(Self {
            priority: tokens[0].parse().map_err(|_| "Failed to parse priority")?,
            allow: tokens[1] == "permit",
            prefix: IpCidr::from_str(tokens[2]).map_err(|_| "Invalid CIDR")?,
            min_len: tokens[3].parse().map_err(|_| "Failed to parse min_len")?,
            max_len: tokens[4].parse().map_err(|_| "Failed to parse max_len")?,
        })
    }
}


pub fn evaluate_filter_set(object_list: &mut Vec<RouteObject>, filter_set: &[Filter]) {
    object_list.retain_mut(|v| {
        let mut filter_set_iter = filter_set.iter();
        let bits: u8 = v.prefix.network_length();

        let applicable_filter_set = filter_set_iter.find(|f| {
            f.prefix.contains(&v.prefix.first_address()) && f.prefix.contains(&v.prefix.last_address())
        });

        match applicable_filter_set {
            None => false,
            Some(filter) => {
                if !filter.allow {
                    return false;
                }

                let new_max = if let Some(current_max) = v.max_length {
                    current_max.clamp(filter.min_len, filter.max_len)
                } else {
                    filter.max_len
                };

                v.max_length = Some(new_max);

                bits <= new_max
            }
        }
    })
}

pub fn read_filter_set(file: &Path) -> BoxResult<(Vec<Filter>, Vec<String>)> {
    let mut warnings: Vec<String> = Vec::new();
    let mut set: Vec<Filter> = Vec::new();
    let lines = read_lines(file).map_err(|e|
        format!("Error reading filter file: {}", e)
    )?;
    for line_result in lines {
        let line = line_result.map_err(|e|
            format!("Error reading filter line: {}", e)
        )?;
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let entries = line.split_whitespace().collect::<Vec<&str>>();
        let result = Filter::from_tokens(entries.as_slice());
        match result {
            Ok(r) => {
                set.push(r)
            }
            Err(err) => {
                let error_message = format!("Failed to parse filter.txt line: {} Error: {}", line, err);
                warnings.push(error_message);
            }
        }
    }

    set.sort_by(|a, b| a.priority.cmp(&b.priority));
    Ok((set, warnings))
}


#[derive(Debug)]
pub struct RouteObject {
    pub prefix: IpCidr,
    pub origins: Vec<String>,
    pub max_length: Option<u8>,
}

impl RouteObject {
    pub fn get_bird_format(self) -> String {
        let mut result = String::new();
        let prefix = self.get_prefix_string();
        let max_length = self.max_length.unwrap();
        for origin in &self.origins {
            result.push_str(&format!("route {prefix} max {max_length} as {origin};\n", prefix = prefix,
                                     max_length = max_length, origin = origin));
        }
        result
    }
    pub fn get_json_objects(self) -> Vec<JsonValue> {
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

pub fn read_route_objects<P>(path: P, expect_v6: bool) -> BoxResult<RouteObjectsWithWarnings>
where
    P: AsRef<Path>,
{
    #[derive(Debug)]
    struct RouteObjectBuilder {
        filename: String,
        prefix: Option<String>,
        origins: Vec<String>,
        max_length: Option<String>,
    }
    impl RouteObjectBuilder {
        fn new(filename: String) -> Self {
            Self {
                filename,
                prefix: None,
                origins: Vec::new(),
                max_length: None,
            }
        }
        fn validate_and_build(mut self, expect_v6: bool) -> BoxResult<RouteObject> {
            if self.origins.is_empty() {
                return Err("missing origin field in object")?;
            }

            for origin in &mut self.origins {
                let clean_origin = origin.strip_prefix("AS").ok_or("Invalid origin filed")?;
                if !clean_origin.chars().all(char::is_numeric) {
                    return Err(format!("Invalid origin field: {}", origin).into());
                }
                *origin = clean_origin.to_string();
            }

            self.origins.sort_unstable();
            self.origins.dedup();

            if self.prefix.is_none() {
                return Err("missing route or route6 field in object")?;
            }
            if self.filename.replace('_', "/") != self.prefix.as_deref().unwrap() {
                return Err("filename does not equal prefix field")?;
            }
            let prefix = IpCidr::from_str(&self.prefix.unwrap()).map_err(|e|
                format!("Unable to parse IP CIDR: {}", e)
            )?;

            if prefix.is_ipv4() && expect_v6 {
                return Err("expected IPv6 but found an IPv4 object")?;
            } else if prefix.is_ipv6() && !expect_v6 {
                return Err("expected IPv4 but found an IPv6 object")?;
            }


            let max_length = self.max_length.map_or(Ok(None), |s|
                if let Ok(parsed) = s.parse::<u8>() {
                    Ok(Some(parsed))
                } else {
                    Err("Failed to parse max_length value as u8")
                },
            )?;

            let result = RouteObject {
                prefix,
                origins: self.origins,
                max_length,
            };
            Ok(result)
        }
    }

    let mut objects: Vec<RouteObject> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let dir = read_dir(path.as_ref()).map_err(|e|
        format!("Unable to read directory {}: {}", path.as_ref().display(), e)
    )?;
    for file_result in dir {
        let file = file_result.map_err(|e|
            format!("Unable to read directory file {}: {}", path.as_ref().display(), e)
        )?.path();
        let lines = read_lines(&file).map_err(|e|
            format!("Unable to open file {}: {}", file.display(), e)
        )?;
        let filename = file.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_owned();
        let mut object = RouteObjectBuilder::new(filename.to_owned());
        for line in lines {
            let line = line.map_err(|e|
                format!("Unable to read file line {}: {}", file.display(), e)
            )?;
            if !line.starts_with(' ') && let Some((key,value)) = line.split_once(':') {
                let val = value.trim_end().to_owned();
                match key.trim_end() {
                    "route" | "route6" => { object.prefix = Some(val) }
                    "origin" => { object.origins.push(val) }
                    "max-length" => { object.max_length = Some(val) }
                    &_ => {}
                }
            }
        }
        match object.validate_and_build(expect_v6) {
            Ok(result) => {
                objects.push(result);
            }
            Err(err) => {
                let error_message = format!("Error in file: {}: {}", filename, err);
                warnings.push(error_message);
            }
        }
    };
    Ok((objects, warnings))
}


fn read_lines<P>(path: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(path)?;
    Ok(io::BufReader::new(file).lines())
}