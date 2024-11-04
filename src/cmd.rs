use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command};
use crate::{NAME, VERSION};
use crate::modules::util::EitherOr;

pub fn get_arg_matches() -> ArgMatches {
    Command::new(NAME)
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
                    Command::new("zones-legacy").about("Output zone files (legacy format)")
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
                        .help("object type such as 'mntner', 'dns' etc. \
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
                .about("Object output with forward and backlinks, path between objects, related objects (JSON / graphviz dot format)")
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
                    Command::new("path")
                        .about("Output the shortest path found between two registry objects (if one exists)")
                        .args([
                            Arg::new("src_object_type")
                                .required(true)
                                .help("Source object type (i.e. aut-num)"),
                            Arg::new("src_object_name")
                                .required(true)
                                .help("Source object name (i.e. AS4242420000)"),
                            Arg::new("tgt_object_type")
                                .required(true)
                                .help("Target object type (i.e. aut-num)"),
                            Arg::new("tgt_object_name")
                                .required(true)
                                .help("Target object name (i.e. AS4242420001)"),
                        ])
                ]),
            Command::new("hierarchical_prefixes")
                .about("Hierarchical prefix tree output (JSON format)")
                .subcommand_required(true)
                .subcommands([
                    Command::new("v4").about("IPv4"),
                    Command::new("v6").about("IPv6"),
                ]),
            #[cfg(feature = "explorer")]
            Command::new("explorer")
                .about("Start web-based registry explorer (including a ROA file server)")
                .args([
                    Arg::new("port")
                        .long("port")
                        .short('p')
                        .value_parser(clap::value_parser!(u16))
                        .default_value("8080")
                        .help("Port to listen on"),
                    Arg::new("disable-roa")
                        .help("Disable ROA API endpoint")
                        .long("disable-roa")
                        .action(ArgAction::SetTrue),
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
    ).get_matches()
}
pub fn get_input_list(c: &ArgMatches) -> EitherOr<String, String> {
    if c.contains_id("list_file") {
        let list_file = c.get_one::<String>("list_file").unwrap();
        EitherOr::A(list_file.clone())
    } else {
        let list = c.get_one::<String>("list").unwrap();
        EitherOr::B(list.clone())
    }
}