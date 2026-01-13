#![forbid(unsafe_code)]

//! Library for parsing DN42 registry route objects and generating ROA data in various formats:
//! - JSON
//! - Bird v4
//! - Bird v6

/// Underlying parser types
pub mod parse;
/// All returned error types
pub mod errors;
mod output;
mod filter;
mod util;


use crate::errors::GenerationError;
use crate::filter::{evaluate_filter_set, read_filter_set};
use crate::parse::{read_route_objects, RouteObject};
use std::path::Path;
use std::sync::Mutex;
use std::thread;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Library name
pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");


/// The function receives a [GenerationError] and should return [WarningAction::ActionContinue]
/// if the process should continue, or [WarningAction::ActionAbort] to abort.
pub trait WarningHandler: FnMut(GenerationError) -> WarningAction {}
impl<T> WarningHandler for T where T: FnMut(GenerationError) -> WarningAction {}

/// Action passed to the [WarningHandler]
#[derive(PartialEq, Copy, Clone)]
pub enum WarningAction {
    /// Abort
    ActionAbort,
    /// Continue
    ActionContinue
}

/// Resulting processed ROA data
pub struct RoaData (Vec<RouteObject>);
impl RoaData {
    /// Access the underling [RouteObject] list
    pub fn object_list(&self) -> &Vec<RouteObject> {
        &self.0
    }
    /// Merge two [RoaData] instances. Used for merging v4-only and v6-only instances
    pub fn merge(&mut self, mut other: Self) {
        self.0.append(&mut other.0);
    }
}

/// Same as [get_roa_data_v4v6], however data is also merged
pub fn get_roa_data_combined<F>(base_path: impl AsRef<Path>, on_warning: F) -> Result<RoaData, GenerationError>
where F: WarningHandler + Send {
    let (mut v4, v6) = get_roa_data_v4v6(base_path, on_warning)?;
    v4.merge(v6);
    Ok(v4)
}
/// Get roa data for both address families at the same time. This function uses threads to parallelize the generation.
pub fn get_roa_data_v4v6<F>(base_path: impl AsRef<Path>, on_warning: F) -> Result<(RoaData, RoaData), GenerationError>
where F: WarningHandler + Send  {

    let base_path = base_path.as_ref();

    let on_warning_mutex = Mutex::new(on_warning);

    let (result_v4, result_v6) = thread::scope(|s| {
        let h1 = s.spawn(|| {
            get_roa_data(false, base_path, |err| {
                let mut guard = on_warning_mutex.lock().unwrap();
                (*guard)(err)
            })
        });
        let h2 = s.spawn(|| {
            get_roa_data(true, base_path, |err| {
                let mut guard = on_warning_mutex.lock().unwrap();
                (*guard)(err)
            })
        });
        (h1.join().unwrap(), h2.join().unwrap())
    });

    Ok((result_v4?, result_v6?))
}

/// Get ROA data for a specific address family
pub fn get_roa_data<F>(is_v6: bool, base_path: impl AsRef<Path>, mut on_warning: F) -> Result<RoaData, GenerationError>
where F: WarningHandler
{
    let base = base_path.as_ref();

    let (route_directory, filter_txt) = if is_v6 {
        (base.join("data/route6/"), base.join("data/filter6.txt"))
    } else {
        (base.join("data/route/"), base.join("data/filter.txt"))
    };

    let mut objects = read_route_objects(route_directory, is_v6, &mut on_warning)?;
    let filters = read_filter_set(&filter_txt, on_warning)?;

    evaluate_filter_set(objects.as_mut(), filters.as_ref());
    Ok(RoaData(objects))
}
