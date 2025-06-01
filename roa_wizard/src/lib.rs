mod parse;
mod output;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::exit;
use crate::output::{output_bird, output_json};
use crate::parse::{evaluate_filter_set, read_filter_set, read_route_objects, RouteObject};
use std::{io, thread};
use std::io::Write;
use std::thread::JoinHandle;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
type Warnings = Vec<String>;
type RouteObjectsWithWarnings = (Vec<RouteObject>, Warnings);
type BoxResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub fn generate_bird(base_path: impl AsRef<Path>, is_v6: bool) -> BoxResult<(String, Warnings)> {
    let result = get_roa_objects(is_v6, base_path.as_ref().into());
    let (objects, warnings) = result?;
    Ok((output_bird(objects, base_path.as_ref()), warnings))
}

pub fn generate_json(base_path: impl AsRef<Path>) -> BoxResult<(String, Warnings)> {
    let handler_v4 = process_handler(false, base_path.as_ref().into());
    let handler_v6 = process_handler(true, base_path.as_ref().into());
    let f_result_v4 = handler_v4.join().expect("thread failed");
    let f_result_v6 = handler_v6.join().expect("thread failed");

    let (mut result_v4, mut warnings_v4) = f_result_v4?;
    let (mut result_v6, mut warnings_v6) = f_result_v6?;

    result_v4.append(result_v6.as_mut());
    warnings_v4.append(warnings_v6.as_mut());
    Ok((output_json(result_v4), warnings_v4))
}

pub fn get_roa_objects(is_v6: bool, base_path: PathBuf) -> BoxResult<RouteObjectsWithWarnings> {
    let route_directory: PathBuf;
    let filter_txt: PathBuf;
    match is_v6 {
        true => {
            let route6_directory = base_path.join("data/route6/");
            let filter6_txt = base_path.join("data/filter6.txt");
            route_directory = route6_directory;
            filter_txt = filter6_txt;
        }
        false => {
            let route4_directory = base_path.join("data/route/");
            let filter4_txt = base_path.join("data/filter.txt");
            route_directory = route4_directory;
            filter_txt = filter4_txt;
        }
    }
    let (mut objects, mut warnings) = read_route_objects(route_directory, is_v6)?;
    let (filters, mut warnings_filter) = read_filter_set(&filter_txt)?;
    warnings.append(&mut warnings_filter);

    evaluate_filter_set(objects.as_mut(), filters.as_ref());
    Ok((objects, warnings))
}

fn process_handler(is_v6: bool, base_path: PathBuf) -> JoinHandle<BoxResult<RouteObjectsWithWarnings>> {
    thread::spawn(move || {
        get_roa_objects(is_v6, base_path)
    })
}

pub fn check_and_output(result: BoxResult<(String, Warnings)>, strict: bool) {
    if let Err(err) = result {
        eprintln!("Error: {}", err);
        exit(1)
    }
    let (output, warnings) = result.unwrap();
    let mut had_warning: bool = false;
    for warning in warnings {
        eprintln!("Warning: {}", warning);
        had_warning = true;
    }
    if strict && had_warning {
        eprintln!("Warnings occurred and strict mode is enabled");
        exit(1)
    }
    write!(io::stdout(), "{}", output).ok();
}
