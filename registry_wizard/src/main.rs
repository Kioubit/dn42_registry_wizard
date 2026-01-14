#![forbid(unsafe_code)]
use crate::modules::registry_remove::RemovalCategory;
use crate::modules::util::BoxResult;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;

mod modules;
mod cmd;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");


fn main() {
    let cmd = cmd::get_arg_matches();
    let base_path = cmd.get_one::<String>("registry_root").unwrap().to_owned();
    let base_path = PathBuf::from(base_path);

    match cmd.subcommand() {
        Some(("roa", c)) => {
            let is_strict = *c.get_one::<bool>("strict").unwrap();
            let warning_action = if is_strict {
                roa_wizard::WarningAction::ActionAbort
            } else {
                roa_wizard::WarningAction::ActionContinue
            };
            let warning_handler = |warning| {
                eprintln!("Warning: {}", warning);
                warning_action
            };

            match c.subcommand() {
                Some(("v4", _)) => {
                    output_result(
                        roa_wizard::get_roa_data(false, &base_path, warning_handler)
                            .map(|x| x.output_bird(base_path))
                            .map_err(Into::into),
                    )
                }
                Some(("v6", _)) => {
                    output_result(
                        roa_wizard::get_roa_data(true, &base_path, warning_handler)
                            .map(|x| x.output_bird(base_path))
                            .map_err(Into::into),
                    )
                }
                Some(("json", _)) => {
                    output_result(
                        roa_wizard::get_roa_data_combined(&base_path, warning_handler)
                            .map(|x| x.output_json())
                            .map_err(Into::into),
                    )
                }
                _ => unreachable!(),
            }
        }
        Some(("dns", c)) => {
            match c.subcommand() {
                Some(("zones", d)) => {
                    let auth_servers: Vec<String> = d.get_many("authoritative_servers").unwrap().cloned().collect();
                    output_result(modules::zone_files::output_forward_zones(&base_path, auth_servers));
                }
                Some(("zones-legacy", d)) => {
                    let auth_servers: Vec<String> = d.get_many("authoritative_servers").unwrap().cloned().collect();
                    output_result(modules::zone_files::output_forward_zones_legacy(&base_path, auth_servers));
                }
                Some(("tas", _)) => {
                    output_result(modules::zone_files::output_tas(&base_path, false));
                }
                Some(("tas-yml", _)) => {
                    output_result(modules::zone_files::output_tas(&base_path, true));
                }
                _ => unreachable!()
            }
        }
        Some(("object_metadata", c)) => {
            let skip_empty = *c.get_one::<bool>("skip_empty").unwrap();
            let object_type = c.get_one::<String>("object_type").unwrap().clone();
            let mut filtered_fields: Option<Vec<String>> = None;
            let mut exclusive_fields: Option<Vec<String>> = None;
            if c.contains_id("filtered_fields") {
                let v = c.get_one::<String>("filtered_fields").unwrap().clone();
                let l: Vec<_> = v.split(",").map(|v| v.to_string()).collect();
                filtered_fields = Some(l);
            }
            if c.contains_id("exclusive_fields") {
                let v = c.get_one::<String>("exclusive_fields").unwrap().clone();
                let l: Vec<_> = v.split(",").map(|v| v.to_string()).collect();
                exclusive_fields = Some(l);
            }
            let result = modules::object_metadata::output(
                &base_path, &object_type, exclusive_fields, filtered_fields, skip_empty,
            );
            output_result(result)
        }
        Some(("object_finder", c)) => {
            match c.subcommand() {
                Some(("ip", d)) => {
                    let search_ip =  d.get_one::<String>("address").unwrap().clone();
                    output_result(modules::object_finder::output(&base_path, &search_ip));
                }
                _ => unreachable!()
            }
        },
        Some(("graph", c)) => {
            match c.subcommand() {
                Some(("list", c)) => {
                    let mut obj_type: Option<String> = None;
                    let mut obj_name: Option<String> = None;
                    let graphviz = *c.get_one::<bool>("graphviz").unwrap();
                    if c.contains_id("object_type") {
                        obj_type = Some(c.get_one::<String>("object_type").unwrap().clone());
                    }
                    if c.contains_id("object_name") {
                        obj_name = Some(c.get_one::<String>("object_name").unwrap().clone());
                    }

                    let result = modules::registry_graph_tools::output_list(&base_path, obj_type, obj_name, graphviz);
                    output_result(result)
                }
                Some(("related", c)) => {
                    let obj_type = c.get_one::<String>("object_type").unwrap().clone();
                    let obj_name = c.get_one::<String>("object_name").unwrap().clone();
                    let enforce_mnt_by = if c.contains_id("enforce_mnt_by") {
                        Some(c.get_one::<String>("enforce_mnt_by").unwrap().clone())
                    } else {
                        None
                    };
                    let related_mnt_by = if c.contains_id("related_mnt_by") {
                        Some(c.get_one::<String>("related_mnt_by").unwrap().clone())
                    } else {
                        None
                    };
                    let contain_value = if c.contains_id("contain_value") {
                        Some(c.get_one::<String>("contain_value").unwrap().clone())
                    } else {
                        None
                    };
                    let not_contain_value = if c.contains_id("not_contain_value") {
                        Some(c.get_one::<String>("not_contain_value").unwrap().clone())
                    } else {
                        None
                    };
                    let graphviz = *c.get_one::<bool>("graphviz").unwrap();
                    let no_cross_dn42_mnt = *c.get_one::<bool>("no_cross_dn42_mnt").unwrap();
                    let result = modules::registry_graph_tools::output_related(&base_path, obj_type, obj_name, enforce_mnt_by, related_mnt_by, not_contain_value, contain_value, graphviz, no_cross_dn42_mnt);
                    output_result(result)
                }
                Some(("path", c)) => {
                    let src_type = c.get_one::<String>("src_object_type").unwrap().clone();
                    let tgt_type = c.get_one::<String>("tgt_object_type").unwrap().clone();
                    let src_name = c.get_one::<String>("src_object_name").unwrap().clone();
                    let tgt_name = c.get_one::<String>("tgt_object_name").unwrap().clone();
                    let result = modules::registry_graph_tools::output_path(&base_path, src_type, tgt_type, src_name, tgt_name);
                    output_result(result)
                }
                _ => unreachable!()
            }
        }
        Some(("hierarchical_prefixes", c)) => {
            let result = match c.subcommand() {
                Some(("v4", _)) => {
                    modules::hierarchical_prefixes::output(&base_path, true)
                }
                Some(("v6", _)) => {
                    modules::hierarchical_prefixes::output(&base_path, false)
                }
                _ => unreachable!()
            };
            output_result(result)
        }
        #[cfg(feature = "explorer")]
        Some(("explorer", c)) => {
            use crate::modules::explorer::start_explorer;
            let port = *c.get_one::<u16>("port").unwrap();
            let disable_roa = *c.get_one::<bool>("disable-roa").unwrap();
            let result = start_explorer(&base_path, port, !disable_roa);
            output_result(result);
        }
        #[cfg(feature = "rtr-server")]
        Some(("rtr", c)) => {
            use crate::modules::rtr::start_rtr;
            let port = *c.get_one::<u16>("port").unwrap();
            let refresh = *c.get_one::<u32>("refresh").unwrap();
            let retry = *c.get_one::<u32>("retry").unwrap();
            let expire = *c.get_one::<u32>("expire").unwrap();
            let bind_ip = c.get_one::<String>("bind-ip").cloned();
            let result = start_rtr(&base_path, port, bind_ip, refresh, retry, expire);
            output_result(result);
        }
        Some(("remove", c)) => {
            let result = match c.subcommand() {
                Some(("mnt", c)) => {
                    let input = cmd::get_input_list(c);
                    let enable_subgraph_check = *c.get_one::<bool>("enable_subgraph_check").unwrap();
                    modules::registry_remove::output(&base_path, input, RemovalCategory::Mnt, enable_subgraph_check)
                }
                Some(("aut-num", c)) => {
                    let input = cmd::get_input_list(c);
                    let enable_subgraph_check = *c.get_one::<bool>("enable_subgraph_check").unwrap();
                    modules::registry_remove::output(&base_path, input, RemovalCategory::Asn, enable_subgraph_check)
                }
                _ => unreachable!()
            };
            output_result(result)
        }
        Some(("mrt_activity", c)) => {
            let result = match c.subcommand() {
                Some(("parse", c)) => {
                    let cutoff_time = *c.get_one::<u64>("cutoff_time").unwrap();
                    let mrt_root = c.get_one::<String>("mrt_root").unwrap().clone();
                    let list_output = *c.get_one::<bool>("list_output").unwrap();
                    modules::mrt_activity::output(mrt_root, cutoff_time, list_output)
                }
                Some(("active_asn_to_inactive", c)) => {
                    let input = cmd::get_input_list(c);
                    let cutoff_time = c.get_one::<u64>("cutoff_time").cloned();
                    modules::inactive_asns::output(&base_path, input, cutoff_time)
                }
                _ => unreachable!()
            };
            output_result(result)
        }
        _ => unreachable!()
    }
}


fn output_result(result: BoxResult<String>) {
    match result {
        Ok(s) => {
            write!(io::stdout(), "{}", s).ok()
        }
        Err(err) => {
            writeln!(io::stderr(), "{}", err).ok();
            exit(1);
        }
    };
}
