use crate::modules::util::{BoxResult, EitherOr};
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command};
use roa_wizard_lib::{generate_bird, generate_json};
use std::io;
use std::io::Write;
use std::process::exit;
use crate::modules::registry_remove::RemovalCategory;

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
                .about("Registry object output with forward and backlinks (JSON / graphviz dot format)")
                .subcommand_required(true)
                .subcommands([
                    Command::new("list")
                        .about("List graph or specified parts of it")
                        .args([
                            Arg::new("object_type")
                                .help("Only output specific object types (i.e. aut-num)"),
                            Arg::new("object_name")
                                .help("Only output a specific object by name"),
                            Arg::new("graphviz")
                                .help("Output graphviz dot")
                                .long("graphviz")
                                .short('g')
                                .action(ArgAction::SetTrue),
                        ]),
                    Command::new("related")
                        .about("Show all related objects to a specified one")
                        .args([
                            Arg::new("object_type")
                                .required(true)
                                .help("Object types (i.e. aut-num)"),
                            Arg::new("object_name")
                                .required(true)
                                .help("Object name (i.e. AS4242420000)"),
                            Arg::new("enforce_mnt_by")
                                .help("Only show objects that are maintained by the specified mnt")
                                .long("enforce-mnt-by")
                                .short('e'),
                            Arg::new("related_mnt_by")
                                .help("Only show objects that are maintained by the specified mnt or that are directly related")
                                .long("related-mnt-by")
                                .short('r'),
                            Arg::new("graphviz")
                                .help("Output graphviz dot")
                                .long("graphviz")
                                .short('g')
                                .action(ArgAction::SetTrue),
                        ])
                        .group(ArgGroup::new("input_group")
                            .args(["enforce_mnt_by", "related_mnt_by"])
                            .required(false)),
                ]),
            Command::new("hierarchical_prefixes")
                .about("Hierarchical prefix tree output (JSON format)")
                .subcommand_required(true)
                .subcommands([
                    Command::new("v4").about("IPv4"),
                    Command::new("v6").about("IPv6"),
                ]),
            Command::new("remove")
                .about("Safely remove a list of registry objects along with all their dependencies")
                .subcommand_required(true)
                .subcommands([
                    Command::new("mnt")
                        .about("Remove a list of maintainers along with all their objects")
                        .args([
                            Arg::new("list_file")
                                .long("list_file")
                                .short('f')
                                .help("Path to a file containing a comma-separated list of maintainers to remove"),
                            Arg::new("list")
                                .long("list")
                                .short('l')
                                .help("Comma-separated list of maintainers to remove"),
                            Arg::new("enable_subgraph_check")
                                .help("Enable check for invalid sub-graphs")
                                .long("enable_subgraph_check")
                                .short('s')
                                .action(ArgAction::SetTrue),
                        ])
                        .group(
                            ArgGroup::new("input_group")
                                .args(["list_file", "list"])
                                .required(true)
                        ),
                    Command::new("aut-num")
                        .about("Remove a list of unused aut-nums along with dependencies")
                        .args([
                            Arg::new("list_file")
                                .long("list_file")
                                .short('f')
                                .help("Path to a file containing a comma-separated list of aut-nums to remove"),
                            Arg::new("list")
                                .long("list")
                                .short('l')
                                .help("Output a comma-separated list of aut-nums to remove"),
                            Arg::new("enable_subgraph_check")
                                .help("Enable check for invalid sub-graphs")
                                .long("enable_subgraph_check")
                                .short('s')
                                .action(ArgAction::SetTrue),
                        ])
                        .group(
                            ArgGroup::new("input_group")
                                .args(["list_file", "list"])
                                .required(true)
                        ),
                ]),
            Command::new("mrt_activity")
                .about("Output active ASNs from MRT RIB dumps along with their last seen time")
                .subcommand_required(true)
                .subcommands([
                    Command::new("parse")
                        .about("Parse MRT RIB dumps")
                        .args([
                            Arg::new("mrt_root")
                                .help("Path to the MRT data directory")
                                .required(true),
                            Arg::new("cutoff_time")
                                .help("Earliest unix time at which an ASN is considered to be active")
                                .default_value("0")
                                .short('c')
                                .long("cutoff-time")
                                .value_parser(clap::value_parser!(u64)),
                            Arg::new("list_output")
                                .help("Output plain comma-separated list instead of JSON")
                                .long("list")
                                .short('l')
                                .action(ArgAction::SetTrue),
                        ]),
                    Command::new("active_asn_to_inactive")
                        .about("Convert a list of active ASNs to a list of inactive ASNs by \
                        looking through the registry. Optionally check git log for activity.")
                        .args([
                            Arg::new("list_file")
                                .long("list_file")
                                .short('f')
                                .help("Path to a file containing a comma-separated list of aut-nums to keep"),
                            Arg::new("list")
                                .long("list")
                                .short('l')
                                .help("Comma-separated list of aut-nums to keep"),
                            Arg::new("cutoff_time")
                                .help("Earliest unix time at which an ASN is considered to be active. (Checked using git log)")
                                .short('c')
                                .long("cutoff-time")
                                .value_parser(clap::value_parser!(u64)),
                        ])
                        .group(
                            ArgGroup::new("input_group")
                                .args(["list_file", "list"])
                                .required(true)
                        ),
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
                base_path, object_type, exclusive_fields, filtered_fields, skip_empty,
            );
            output_result(result)
        }
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

                    let result = modules::registry_graph::output_list(base_path, obj_type, obj_name, graphviz);
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
                    let graphviz = *c.get_one::<bool>("graphviz").unwrap();
                    let result = modules::registry_graph::output_related(base_path, obj_type, obj_name, enforce_mnt_by, related_mnt_by, graphviz);
                    output_result(result)
                }
                _ => unreachable!()
            }
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
        Some(("remove", c)) => {
            let result = match c.subcommand() {
                Some(("mnt", c)) => {
                    let input = get_input_list(c);
                    let enable_subgraph_check = *c.get_one::<bool>("enable_subgraph_check").unwrap();
                    modules::registry_remove::output(base_path, input, RemovalCategory::Mnt, enable_subgraph_check)
                }
                Some(("aut-num", c)) => {
                    let input = get_input_list(c);
                    let enable_subgraph_check = *c.get_one::<bool>("enable_subgraph_check").unwrap();
                    modules::registry_remove::output(base_path, input, RemovalCategory::Asn, enable_subgraph_check)
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
                    let input = get_input_list(c);
                    let cutoff_time = c.get_one::<u64>("cutoff_time").cloned();
                    modules::inactive_asns::output(base_path, input, cutoff_time)
                }
                _ => unreachable!()
            };
            output_result(result)
        }
        _ => unreachable!()
    }
}

fn get_input_list(c: &ArgMatches) -> EitherOr<String, String> {
    if c.contains_id("list_file") {
        let list_file = c.get_one::<String>("list_file").unwrap();
        EitherOr::A(list_file.clone())
    } else {
        let list = c.get_one::<String>("list").unwrap();
        EitherOr::B(list.clone())
    }
}

fn output_result(result: BoxResult<String>) {
    if result.is_err() {
        writeln!(io::stderr(), "{}", result.unwrap_err()).ok();
        exit(1);
    }
    writeln!(io::stdout(), "{}", result.unwrap()).ok();
}