use clap::{Arg, ArgAction, Command};
use roa_wizard_lib::{generate_bird, generate_json};
use std::process::exit;

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
        )
        .subcommands([
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
                                             .help("List of authoritative servers")
                                             .required(true)
                                             .num_args(1..)
                                     ),
                                 Command::new("zones-legacy")
                                     .about("Output legacy zone files")
                                     .arg(
                                         Arg::new("authoritative_servers")
                                             .help("List of authoritative servers")
                                             .required(true)
                                             .num_args(1..)
                                     ),
                                 Command::new("tas").about("Output trust anchors"),
                             ]),
                         Command::new("inetnumMetadata")
                             .about("Inetnum metadata output (JSON format)")
                             .subcommand_required(true)
                             .subcommands([
                                 Command::new("v4").about("IPv4"),
                                 Command::new("v6").about("IPv6"),
                             ]),
                         Command::new("objectMetadata")
                             .about("Object metadata output (JSON format)")
                             .arg(
                                 Arg::new("object_type")
                                     .required(true)
                                     .help("object type such as 'mntner', 'domain' etc. \
                                     (Based on the directories in the registry)")
                             )
                         ,
                         Command::new("hierarchicalPrefixes")
                             .about("Hierarchical prefix tree output (JSON format)")
                             .subcommand_required(true)
                             .subcommands([
                                 Command::new("v4").about("IPv4"),
                                 Command::new("v6").about("IPv6"),
                             ]),
                         Command::new("mrt_activity")
                             .about("Output last seen time for active ASNs in MRT RIB dumps. List registry resources that are unused.")
                             .subcommand_required(true)
                             .subcommands([
                                 Command::new("parse_mrt")
                                     .about("Parse mrt data files from directory")
                                     .arg(
                                         Arg::new("mrt_root")
                                             .help("Path to the MRT data directory")
                                             .required(true)
                                     ),
                                 Command::new("filter_registry")
                                     .about("List unused registry resources")
                                     .arg(
                                         Arg::new("active_json")
                                             .help("Path to the JSON file containing active ASNs (can be generated by the parse_mrt command)")
                                             .required(true)
                                     ),
                             ])
                             .args([
                                 Arg::new("max_inactive_secs")
                                     .help("Minimum age in seconds for an ASN to be considered inactive")
                                     .default_value("0")
                                     .short('i')
                                     .long("max-inactive-secs")
                                     .value_parser(clap::value_parser!(u64)),
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
            println!("Roa generation");
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
                    let auth_servers: Vec<&String> = d.get_many("authoritative_servers").unwrap().collect();
                    modules::zone_files::output_forward_zones(base_path, auth_servers.into_iter().map(|v| v.to_string()).collect());
                }
                Some(("zones-legacy", d)) => {
                    let auth_servers: Vec<&String> = d.get_many("authoritative_servers").unwrap().collect();
                    modules::zone_files::output_forward_zones_legacy(base_path, auth_servers.into_iter().map(|v| v.to_string()).collect());
                }
                Some(("tas", _)) => {
                    modules::zone_files::output_tas(base_path);
                }
                _ => unreachable!()
            }
        }
        Some(("inetnumMetadata", c)) => {
            let result = match c.subcommand() {
                Some(("v4", _)) => {
                    modules::inetnum_metadata::output(base_path, true)
                }
                Some(("v6", _)) => {
                    modules::inetnum_metadata::output(base_path, false)
                }
                _ => unreachable!()
            };
            if result.is_err() {
                println!("{}", result.unwrap_err());
                exit(1);
            }
            println!("{}", result.unwrap());
        }
        Some(("objectMetadata", c )) => {
            let object_type = c.get_one::<String>("object_type").unwrap();
            let result = modules::object_metadata::output(base_path, object_type.to_owned());
            if result.is_err() {
                println!("{}", result.unwrap_err());
                exit(1);
            }
            println!("{}", result.unwrap());
        }
        Some(("hierarchicalPrefixes", c)) => {
            let result = match c.subcommand() {
                Some(("v4", _)) => {
                    modules::hierarchical_prefixes::output(base_path, true)
                }
                Some(("v6", _)) => {
                    modules::hierarchical_prefixes::output(base_path, false)
                }
                _ => unreachable!()
            };
            if result.is_err() {
                println!("{}", result.unwrap_err());
                exit(1);
            }
            println!("{}", result.unwrap());
        }
        Some(("mrt_activity", c)) => {
            let max_inactive_secs = c.get_one::<u64>("max_inactive_secs").unwrap();
            let result = match c.subcommand() {
                Some(("parse_mrt", _)) => {
                    let mrt_root = c.get_one::<String>("mrt_root").unwrap();
                     modules::mrt_activity::output(mrt_root.to_owned(), max_inactive_secs.to_owned())
                }
                Some(("filter_registry", _)) => {
                    let json_file = c.get_one::<String>("active_json").unwrap();
                    modules::mrt_registry::output(base_path, json_file.to_owned(), max_inactive_secs.to_owned())
                }
                _ => unreachable!()
            };
            if result.is_err() {
                println!("{}", result.unwrap_err());
                exit(1);
            }
            println!("{}", result.unwrap());
        }
        _ => {}
    }
}
