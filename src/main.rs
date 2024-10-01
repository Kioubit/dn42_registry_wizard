use std::io::Write;
use std::io;
use clap::{Arg, ArgAction, ArgGroup, Command};
use roa_wizard_lib::{generate_bird, generate_json};
use std::process::exit;
use crate::modules::util::{BoxResult, EitherOr};

mod modules;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");


fn main() {
    let cmd = Command::new(NAME)
        .bin_name(NAME)
        .version(VERSION)
        .about("A collection of tools to interact with DN42 registry data")
        .subcommand_required(true)
        .arg(
            Arg::new("registry_root")
                .help("path to registry root")
                .required(true)
                .index(1)
        ).subcommands(
        [
            Command::new("roa")
                .about("ROA file generation (various formats)")
                .subcommand_required(true)
                .subcommands([
                    Command::new("v4").about("bird2 v4 format"),
                    Command::new("v6").about("bird2 v6 format"),
                    Command::new("json").about("JSON format for use with RPKI"),
                ]).arg(
                Arg::new("strict")
                    .short('s')
                    .long("strict")
                    .action(ArgAction::SetTrue)
                    .help("Abort program if an error was found in a file")
            ),
            Command::new("dns")
                .about("DNS zone file and trust anchor generation (for use with PowerDNS)")
                .subcommand_required(true)
                .subcommands([
                    Command::new("zones").about("Output zone files")
                        .arg(
                            Arg::new("authoritative_servers")
                                .help("List of default authoritative servers (comma separated)")
                                .required(true)
                                .num_args(1..)
                        ),
                    Command::new("tas").about("Output trust anchors"),
                ]),
            Command::new("object_metadata")
                .about("Object metadata output (JSON format)")
                .args([
                    Arg::new("object_type")
                        .required(true)
                        .help("object type such as 'mntner', 'domain' etc. \
                                     (Based on the directories in the registry)"),
                    Arg::new("exclusive_fields")
                        .short('x')
                        .help("Comma separated list of the only object fields to output"),
                    Arg::new("filtered_fields")
                        .short('i')
                        .help("Comma separated list of object fields to ignore"),
                    Arg::new("skip_empty")
                        .short('e')
                        .long("skip_empty")
                        .action(ArgAction::SetTrue)
                        .help("Don't output objects without keys (useful if filtering)"),
                ]),
            Command::new("graph")
                .about("Registry object output with forward and backlinks (JSON format)")
                .args([
                    Arg::new("graph_category")
                        .help("Only output specific object types (i.e. aut-num)"),
                    Arg::new("object_name")
                        .help("Only output a specific object by name")
                    ]),
            Command::new("hierarchical_prefixes")
                .about("Hierarchical prefix tree output (JSON format)")
                .subcommand_required(true)
                .subcommands([
                    Command::new("v4").about("IPv4"),
                    Command::new("v6").about("IPv6"),
                ]),
            Command::new("remove_mnt")
                .about("Remove a list of maintainers along with all their objects from the registry")
                .args([
                    Arg::new("list_file")
                        .long("list_file")
                        .short('f')
                        .help("Path to a file containing a comma-separated list of maintainers to remove"),
                    Arg::new("list")
                        .long("list")
                        .short('l')
                        .help("comma-separated list of maintainers to remove"),
                    Arg::new("enable_subgraph_check")
                        .help("disable check for invalid sub-graphs")
                        .long("enable_subgraph_check")
                        .short('s')
                        .action(ArgAction::SetTrue),
                ])
                .group(
                    ArgGroup::new("input_group")
                        .args(["list_file", "list"])
                        .required(true)
                ),
            Command::new("mrt_activity")
                .about("Output last seen time for active ASNs in MRT RIB dumps. List inactive maintainers.")
                .subcommand_required(true)
                .subcommands([
                    Command::new("parse_mrt")
                        .about("Parse mrt data files from directory")
                        .args([
                            Arg::new("mrt_root")
                                .help("Path to the MRT data directory")
                                .required(true),
                            Arg::new("max_inactive_secs")
                                .help("Minimum age in seconds for an ASN to be considered inactive")
                                .default_value("0")
                                .short('i')
                                .long("max-inactive-secs")
                                .value_parser(clap::value_parser!(u64)),
                        ]),
                    Command::new("inactive_mnt")
                        .about("List inactive MNTs in the registry")
                        .args([
                            Arg::new("active_json")
                                .help("Path to the JSON file containing active ASNs (can be generated by the parse_mrt command)")
                                .required(true),
                            Arg::new("max_inactive_secs")
                                .help("Minimum age in seconds for an ASN to be considered inactive")
                                .default_value("0")
                                .short('i')
                                .long("max-inactive-secs")
                                .value_parser(clap::value_parser!(u64)),
                        ]),
                ])
        ],
    ).get_matches();

    let mut base_path = cmd.get_one::<String>("registry_root").unwrap().to_owned();
    if !base_path.ends_with('/') {
        base_path.push('/');
    }

    match cmd.subcommand() {
        Some(("roa", c)) => {
            let is_strict = c.get_one::<bool>("strict").unwrap();
            match c.subcommand() {
                Some(("v4", _)) => {
                    roa_wizard_lib::check_and_output(generate_bird(base_path, false), *is_strict)
                }
                Some(("v6", _)) => {
                    roa_wizard_lib::check_and_output(generate_bird(base_path, true), *is_strict)
                }
                Some(("json", _)) => {
                    roa_wizard_lib::check_and_output(generate_json(base_path.to_owned()), *is_strict)
                }
                _ => unreachable!(),
            }
        }
        Some(("dns", c)) => {
            match c.subcommand() {
                Some(("zones", d)) => {
                    let auth_servers: Vec<String> = d.get_many("authoritative_servers").unwrap().cloned().collect();
                    output_result(modules::zone_files::output_forward_zones(base_path, auth_servers));
                }
                Some(("tas", _)) => {
                    output_result(modules::zone_files::output_tas(base_path));
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
                base_path, object_type, exclusive_fields, filtered_fields, skip_empty
            );
            output_result(result)
        }
        Some(("graph", c)) => {
            let mut obj_type: Option<String> = None;
            let mut obj_name: Option<String> = None;
            if c.contains_id("graph_category") {
                obj_type = Some(c.get_one::<String>("graph_category").unwrap().clone())
            }
            if c.contains_id("object_name") {
                obj_name = Some(c.get_one::<String>("object_name").unwrap().clone())
            }
            let result = modules::registry_graph::output(base_path, obj_type, obj_name);
            output_result(result)
        }
        Some(("hierarchical_prefixes", c)) => {
            let result = match c.subcommand() {
                Some(("v4", _)) => {
                    modules::hierarchical_prefixes::output(base_path, true)
                }
                Some(("v6", _)) => {
                    modules::hierarchical_prefixes::output(base_path, false)
                }
                _ => unreachable!()
            };
            output_result(result)
        }
        Some(("remove_mnt", c)) => {
            let input: EitherOr<String, String> = if c.contains_id("list_file") {
                let mnt_file = c.get_one::<String>("list_file").unwrap();
                EitherOr::A(mnt_file.clone())
            } else {
                let mnt_list = c.get_one::<String>("list").unwrap();
                EitherOr::B(mnt_list.clone())
            };

            let enable_subgraph_check = *c.get_one::<bool>("enable_subgraph_check").unwrap();
            let result = modules::registry_clean::output(base_path, input, enable_subgraph_check);
            output_result(result)
        }
        Some(("mrt_activity", c)) => {
            let result = match c.subcommand() {
                Some(("parse_mrt", c)) => {
                    let max_inactive_secs = c.get_one::<u64>("max_inactive_secs").unwrap();
                    let mrt_root = c.get_one::<String>("mrt_root").unwrap();
                    modules::mrt_activity::output(mrt_root.to_owned(), max_inactive_secs.to_owned())
                }
                Some(("inactive_mnt", c)) => {
                    let max_inactive_secs = c.get_one::<u64>("max_inactive_secs").unwrap();
                    let json_file = c.get_one::<String>("active_json").unwrap();
                    modules::mrt_registry::output(base_path, json_file.to_owned(), max_inactive_secs.to_owned())
                }
                _ => unreachable!()
            };
            output_result(result)
        }
        _ => unreachable!()
    }
}

fn output_result(result: BoxResult<String>) {
    if result.is_err() {
        writeln!(io::stderr(), "{}", result.unwrap_err()).ok();
        exit(1);
    }
    writeln!(io::stdout(), "{}", result.unwrap()).ok();
}