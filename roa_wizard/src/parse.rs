use crate::{GenerationError, WarningAction, WarningHandler};
use cidr_utils::cidr::IpCidr;
use std::fs::read_dir;
use std::path::Path;
use std::str::FromStr;
use crate::errors::GenerationErrorKind::{CancelledDueToWarning, IoError};
use crate::errors::ParseError;

#[derive(Debug)]
pub struct RouteObject {
    pub prefix: IpCidr,
    pub origins: Vec<String>,
    pub max_length: Option<u8>,
}

pub(crate) fn read_route_objects<P, F>(path: P, expect_v6: bool, mut on_warning: F) -> Result<Vec<RouteObject>, GenerationError>
where
    P: AsRef<Path>,
    F: WarningHandler,
{
    let mut objects: Vec<RouteObject> = Vec::new();
    let dir = read_dir(path.as_ref()).map_err(|e|
        GenerationError::new(IoError(e), path.as_ref().to_str(), None)
    )?;

    for entry in dir {
        let path = entry.map_err(|e|
            GenerationError::new(IoError(e), path.as_ref().to_str(), None)
        )?.path();
        if !path.is_file() {
            continue;
        }

        let filename = path.as_path().file_name().unwrap_or_default().to_str().unwrap_or_default().to_string();

        if let Some(obj) = parse_single_route_file(&path, &filename, expect_v6, &mut on_warning)? {
            objects.push(obj);
        }
    }
    Ok(objects)
}

fn parse_single_route_file<P, F>(path: P, filename: &str, expect_v6: bool, on_warning: &mut F) -> Result<Option<RouteObject>, GenerationError>
where
    P: AsRef<Path>,
    F: WarningHandler,
{
    let lines = crate::util::read_lines(path).map_err(|e|
        GenerationError::new(IoError(e), Some(filename), None)
    )?;

    let mut prefix = None;
    let mut origins = Vec::new();
    let mut max_len_str = None;

    for line in lines {
        let line = line.map_err(|e|
            GenerationError::new(IoError(e), Some(filename), None)
        )?;

        if !line.starts_with(' ') && let Some((key,value)) = line.split_once(':') {
            let val = value.trim();
            match key.trim_end() {
                "route" | "route6" => { prefix = Some(val.to_owned()) }
                "origin" => { origins.push(val.to_owned()) }
                "max-length" => { max_len_str = Some(val.to_owned()) }
                &_ => {}
            }
        }
    }

    // Validation
    let validation_result:  Result<RouteObject, ParseError> = (|| {
        if origins.is_empty() {
            return Err(ParseError::MissingField("origin"))
        }

        for origin in &mut origins {
            if !origin.starts_with("AS") {
                return Err(ParseError::BadOrigin(origin.clone()));
            }

            origin.drain(..2);

            if !origin.chars().all(char::is_numeric) {
                return Err(ParseError::BadOrigin(origin.clone()));
            }
        }

        origins.sort_unstable();
        origins.dedup();

        if prefix.is_none() {
            return Err(ParseError::MissingField("route/route6"));
        }

        if filename.replace('_', "/") != prefix.as_deref().unwrap() {
            return Err(ParseError::BadFileName(filename.to_owned()));
        }

        let prefix = IpCidr::from_str(prefix.as_ref().unwrap()).map_err(|e|
            ParseError::BadPrefix{ prefix_string: prefix.unwrap(), error: e.to_string() }
        )?;

        if prefix.is_ipv4() && expect_v6 {
            return Err(ParseError::TypeMismatch { expected_v6: true });
        } else if prefix.is_ipv6() && !expect_v6 {
            return Err(ParseError::TypeMismatch { expected_v6: false });
        }

        let max_length = max_len_str.map_or(Ok::<Option<u8>, ParseError>(None), |s|
            if let Ok(parsed) = s.parse::<u8>() {
                Ok(Some(parsed))
            } else {
                Err(ParseError::InvalidMaxLen(s))
            },
        )?;

        Ok(RouteObject {
            prefix,
            origins,
            max_length,
        })
    })();

    match validation_result {
        Ok(obj) => Ok(Some(obj)),
        Err(e) => {
            if on_warning(GenerationError::new(e.into(), Some(filename), None)) == WarningAction::ActionContinue{
                Ok(None)
            } else {
                Err(GenerationError::new(CancelledDueToWarning(), None::<String>, None))
            }
        }
    }
}
