use crate::modules::registry_graph::{LinkInfoSchemaKey, LinkedRegistryObject};
use crate::modules::util::BoxResult;
use std::borrow::Cow;
use std::rc::Rc;
use crate::modules::object_reader::SimpleObjectLine;

type Nd = Rc<LinkedRegistryObject<(), SimpleObjectLine, LinkInfoSchemaKey>>;
type Ed = (Nd, Nd);
struct Graph {
    nodes: Vec<Nd>,
    mnt: Option<String>
}

impl<'a> dot::GraphWalk<'a, Nd, Ed > for Graph {
    fn nodes(&'a self) -> dot::Nodes<'a, Nd> {
        Cow::Borrowed(&self.nodes[..])
    }

    fn edges(&'a self) -> dot::Edges<'a, Ed>  {
        let mut edges: Vec<Ed> = Vec::new();
        for node in &self.nodes {
            let links: Vec<_> = node.get_forward_links()
                .chain(node.get_back_links()).collect();
            for link in links {
                let mut found = false;
                for item in self.nodes().iter() {
                    if Rc::ptr_eq(item, &link.1) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    continue;
                }
                edges.push((node.clone(), link.1));
            }
        }
        Cow::Owned(edges)
    }

    fn source(&'a self, edge: &Ed) -> Nd {
        edge.0.clone()
    }

    fn target(&'a self, edge: &Ed) -> Nd {
        edge.1.clone()
    }
}

impl<'a> dot::Labeller<'a, Nd, Ed> for Graph {
    fn graph_id(&'a self) -> dot::Id<'a> { dot::Id::new("graph1").unwrap() }

    fn node_id(&'a self, n: &Nd) -> dot::Id<'a> {
        let name = format!("{}/{}", &n.category ,&n.object.filename);
        let f = name.as_bytes().iter().fold(String::new(), |mut acc, &x| {
            acc.push_str(&format!("{:02x}", x));
            acc
        });
        let id_str = format!("N{}", f);

        dot::Id::new(id_str).unwrap()
    }
    fn node_label(&'a self, n: &Nd) -> dot::LabelText<'a> {
        dot::LabelText::LabelStr(format!("{}/{}", n.category, n.object.filename).into())
    }
    fn node_color(&'a self, n: &Nd) -> Option<dot::LabelText<'a>> {
        n.object.key_value.get("mnt-by").and_then(|mnt_list| {
            if mnt_list.contains(self.mnt.as_ref()?) {
               return Some(dot::LabelText::LabelStr("red".into()));
            }
            None
        })
    }

}

pub fn create_graphviz(a: Vec<Nd>, mnt: Option<String>) -> BoxResult<String> {
    let mut buffer = Vec::new();
    dot::render(&Graph{nodes: a, mnt}, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}