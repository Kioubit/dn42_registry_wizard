use std::process::exit;
use roa_wizard_lib::{check_and_output, generate_bird, generate_json, PACKAGE_NAME, VERSION};

fn show_usage() {
    println!("{} {}", PACKAGE_NAME, VERSION);
    println!("Usage: <path to registry root> <action> [flag]");
    println!();
    println!("Where <action>:");
    println!("'v4' : bird2 v4 format");
    println!("'v6' : bird2 v6 format");
    println!("'json' : json format");
    println!();
    println!("Where <flag>:");
    println!("'' : No flag");
    println!("'strict' : Abort program if an error was found in a file");
    exit(2)
}

fn main() {
    if std::env::args().len() < 3 {
        println!("Missing commandline arguments");
        show_usage();
    }

    let base_path = std::env::args().nth(1).expect("no registry path given");
    let action = std::env::args().nth(2).expect("no action given");
    let strict = std::env::args().nth(3).unwrap_or_default() == "strict";

    match action.as_str() {
        "v4" => {
            check_and_output(generate_bird(base_path, false), strict);
        }
        "v6" => {
            check_and_output(generate_bird(base_path, true), strict);
        }
        "json" => {
            check_and_output(generate_json(base_path), strict);
        }
        _ => {
            println!("Unknown argument for <action>");
            show_usage();
        }
    }
}



