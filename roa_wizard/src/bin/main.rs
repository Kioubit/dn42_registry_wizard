use roa_wizard::{get_roa_data, get_roa_data_combined, WarningAction, PACKAGE_NAME, VERSION};
use std::io;
use std::io::Write;
use std::process::exit;

fn show_usage() -> ! {
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
    let action_arg = std::env::args().nth(2).expect("no action given");
    let strict = std::env::args().nth(3).unwrap_or_default() == "strict";
    let warning_action = if strict {
        WarningAction::ActionAbort
    } else {
        WarningAction::ActionContinue
    };

    let warning_handler = |warning| {
        eprintln!("Warning: {}", warning);
        warning_action
    };

    let result = match action_arg.as_str() {
        "v4" | "v6" => {
            let is_v6 = action_arg == "v6";
            get_roa_data(is_v6, &base_path, warning_handler)
                .map(|data| data.output_bird(base_path))
        }
        "json" => {
            get_roa_data_combined(base_path, warning_handler)
                .map(|data| data.output_json())
        }
        _ => {
            println!("Unknown argument for <action>");
            show_usage();
        }
    };

    match result {
        Ok(output) => {
            let _ = write!(io::stdout(), "{}", output);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    }
}

