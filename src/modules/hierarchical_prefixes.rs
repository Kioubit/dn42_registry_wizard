use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::str::FromStr;
use cidr_utils::cidr::IpCidr;
use serde::{Serialize};
use crate::modules::object_reader::read_registry_objects;
use crate::modules::util;

type PrefixTree = Rc<RefCell<HierarchicalPrefix>>;

#[derive(Debug, Serialize)]
struct HierarchicalPrefix<> {
    #[serde(with = "IpCidrDef")]
    cidr: IpCidr,
    length: i32,
    children: Vec<PrefixTree>,
}

#[derive(Serialize)]
#[serde(remote = "IpCidr")]
struct IpCidrDef (
    #[serde(getter = "IpCidr::to_string")]
    String
);


pub fn output(registry_root: String, v4: bool) -> util::BoxResult<String> {
    let inetnum_path: &str = if v4 {
        "data/inetnum/"
    } else {
        "data/inet6num/"
    };


    let objects = read_registry_objects(registry_root, inetnum_path, true)?;
    let mut cidr_list: Vec<PrefixTree> = Vec::new();
    for object in objects {
        let inetnum = object.filename.replace('_', "/");
        let length = inetnum.split('/').nth(1).ok_or("Could not get length for inetnum: ".to_owned() + &inetnum)?.parse::<i32>()?;
        let cidr = IpCidr::from_str(&inetnum)?;
        cidr_list.push(Rc::new(RefCell::new(HierarchicalPrefix {
            cidr,
            length,
            children: Vec::new(),
        })));
    }


    for current_cidr in &cidr_list {
        let mut parent: Option<RefMut<HierarchicalPrefix>> = None;
        let mut max_compare_length = -1;
        for compare_cidr in &cidr_list {
            if current_cidr.borrow().cidr.eq(&compare_cidr.borrow().cidr) { continue; }
            if compare_cidr.borrow().length > max_compare_length && is_in_subnet(compare_cidr.borrow().cidr, current_cidr.borrow().cidr) {
                max_compare_length = compare_cidr.borrow().length;
                parent = Some(compare_cidr.borrow_mut());
            }
        }

        if let Some(mut parent) = parent {
            parent.children.push(current_cidr.to_owned())
        }
    }

    let root = cidr_list.iter().find(|cidr|
        cidr.borrow().cidr.eq(&IpCidr::from_str("0.0.0.0/0").unwrap()) ||
            cidr.borrow().cidr.eq(&IpCidr::from_str("::/0").unwrap())).
        ok_or("could not find root element")?;


    let j = serde_json::to_string(&root)?;
    Ok(j)
}

fn is_in_subnet(target : IpCidr, test : IpCidr) -> bool {
    target.contains(&test.first_address()) && target.contains(&test.last_address())
}