use crate::parse::RouteObject;
use cidr_utils::cidr::IpCidr;
use std::path::Path;
use std::str::FromStr;
use crate::errors::{FilterSetError, GenerationError, GenerationErrorKind};
use crate::errors::GenerationErrorKind::IoError;
use crate::{WarningAction, WarningHandler};

#[derive(Debug)]
pub struct Filter {
    priority: u32,
    allow: bool,
    prefix: IpCidr,
    min_len: u8,
    max_len: u8,
}

impl Filter {
    fn from_tokens(tokens: &[&str]) -> Result<Self, FilterSetError> {
        if tokens.len() < 5 {
            return Err(FilterSetError::InvalidRow());
        }
        Ok(Self {
            priority: tokens[0].parse().map_err(|_| FilterSetError::InvalidField("priority"))?,
            allow: tokens[1] == "permit",
            prefix: IpCidr::from_str(tokens[2]).map_err(|_| FilterSetError::InvalidField("cidr"))?,
            min_len: tokens[3].parse().map_err(|_| FilterSetError::InvalidField("min_len"))?,
            max_len: tokens[4].parse().map_err(|_| FilterSetError::InvalidField("max_len"))?,
        })
    }
}

pub fn read_filter_set<F>(file: &Path, mut on_warning: F) -> Result<Vec<Filter>, GenerationError>
where F: WarningHandler
{
    let mut set: Vec<Filter> = Vec::new();
    let lines = crate::util::read_lines(file).map_err(|e|
        GenerationError::new(IoError(e), file.to_str(), None)
    )?;
    for (i, line_result) in lines.enumerate() {
        let line = line_result.map_err(|e|
            GenerationError::new(IoError(e), file.to_str(), Some(i))
        )?;
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let entries = line.split_whitespace().collect::<Vec<&str>>();
        let result = Filter::from_tokens(entries.as_slice());
        match result {
            Ok(r) => {
                set.push(r)
            }
            Err(e) => {
                let e = GenerationError::new(e.into(), file.to_str(), Some(i));
                if on_warning(e) == WarningAction::ActionAbort {
                    return Err(GenerationError::new(GenerationErrorKind::CancelledDueToWarning(), None::<String>, None))
                }
            }
        }
    }

    set.sort_by_key(|a| a.priority);
    Ok(set)
}


pub fn evaluate_filter_set(object_list: &mut Vec<RouteObject>, filter_set: &[Filter]) {
    object_list.retain_mut(|v| {
        let mut filter_set_iter = filter_set.iter();
        let bits: u8 = v.prefix.network_length();

        let applicable_filter_set = filter_set_iter.find(|f| {
            f.prefix.contains(&v.prefix.first_address()) && f.prefix.contains(&v.prefix.last_address())
        });

        match applicable_filter_set {
            None => false,
            Some(filter) => {
                if !filter.allow {
                    return false;
                }

                let new_max = if let Some(current_max) = v.max_length {
                    current_max.clamp(filter.min_len, filter.max_len)
                } else {
                    filter.max_len
                };

                v.max_length = Some(new_max);

                bits <= new_max
            }
        }
    })
}