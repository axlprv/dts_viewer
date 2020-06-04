use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use device_tree_source::tree::{DTInfo, Node, NodeName, Offset, Property};

#[derive(Debug)]
pub enum Element<'a> {
    Node(&'a Node),
    Prop(&'a Property),
}

impl<'a> Offset for Element<'a> {
    fn get_offset(&self) -> usize {
        match *self {
            Element::Node(n) => n.get_offset(),
            Element::Prop(p) => p.get_offset(),
        }
    }
}

impl<'a> fmt::Display for Element<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Element::Node(node) => write!(f, "{}", node),
            Element::Prop(prop) => write!(f, "{}", prop),
        }
    }
}

#[derive(Debug)]
pub struct LabelStore<'a> {
    paths: HashMap<PathBuf, Vec<Element<'a>>>,
    labels: HashMap<&'a str, PathBuf>,
}

impl<'a> LabelStore<'a> {
    pub fn new() -> LabelStore<'a> {
        LabelStore {
            paths: HashMap::new(),
            labels: HashMap::new(),
        }
    }

    // TODO: somehow keep track of deleted labels so they can be searched for later
    //       while not being used for path lookup during change parsing
    // TODO: path refs
    pub fn fill(&mut self, dt_info: &'a DTInfo, amends: &'a [Node]) {
        self.fill_internal(Path::new("/"), &dt_info.root);
        for node in amends {
            match *node {
                Node::Existing { ref name, .. } => {
                    if name.as_str() == "/" || self.labels.contains_key(name.as_str()) {
                        self.fill_internal(Path::new(""), node);
                    } else {
                        unimplemented!();
                    }
                }
                Node::Deleted { ref name, .. } => {
                    if self.labels.contains_key(name.as_str()) {
                        self.fill_internal(Path::new(""), node);
                    } else {
                        unimplemented!();
                    }
                }
            }
        }
    }

    fn fill_internal(&mut self, path: &Path, node: &'a Node) {
        match *node {
            Node::Deleted { ref name, .. } => {
                let node_path = match *name {
                    NodeName::Full(ref name) => path.join(name),
                    NodeName::Ref(ref label) => self.path_from_label(label).unwrap().to_owned(),
                };

                self.delete_labels(&node_path);

                self.paths
                    .entry(node_path.clone())
                    .or_insert_with(Vec::new)
                    .push(Element::Node(node));

                let paths: Vec<PathBuf> = self
                    .paths
                    .iter()
                    .filter_map(|(key, val)| {
                        if key.starts_with(&node_path) {
                            match val.last() {
                                Some(&Element::Node(&Node::Existing { .. }))
                                | Some(&Element::Prop(&Property::Existing { .. })) => {
                                    Some(key.to_owned())
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                for path in &paths {
                    self.delete_labels(path);
                    self.paths.get_mut(path).unwrap().push(Element::Node(node));
                }
            }
            Node::Existing {
                ref name,
                ref proplist,
                ref children,
                ref labels,
                ..
            } => {
                let node_path = match *name {
                    NodeName::Full(ref name) => path.join(name),
                    NodeName::Ref(ref label) => self.path_from_label(label).unwrap().to_owned(),
                };

                self.insert_labels(&node_path, labels);

                for (name, prop) in proplist {
                    let label_path = node_path.join(name);

                    match *prop {
                        Property::Deleted { .. } => {
                            self.delete_labels(&label_path);

                            self.paths
                                .entry(label_path)
                                .or_insert_with(Vec::new)
                                .push(Element::Prop(prop));
                        }
                        Property::Existing { ref labels, .. } => {
                            self.insert_labels(&label_path, labels);

                            self.paths
                                .entry(label_path)
                                .or_insert_with(Vec::new)
                                .push(Element::Prop(prop));
                        }
                    }
                }

                for node in children.values() {
                    self.fill_internal(&node_path, node);
                }

                self.paths
                    .entry(node_path)
                    .or_insert_with(Vec::new)
                    .push(Element::Node(node));
            }
        }
    }

    fn delete_labels(&mut self, path: &Path) {
        let mut labels: Vec<&str> = Vec::new();
        for (label, p) in &self.labels {
            if p.starts_with(path) {
                labels.push(label);
            }
        }
        for label in &labels {
            self.labels.remove(label);
        }
    }

    fn insert_labels(&mut self, path: &Path, labels: &'a [String]) {
        for label in labels {
            if !self.labels.contains_key(label.as_str()) {
                self.labels.insert(label, path.to_owned());
            } else if self.labels[label.as_str()] != path {
                // TODO: maybe just print error message and allow user to determine if they should exit
                panic!("Duplicate label \"{}\" at different paths", label);
            }
        }
    }

    pub fn changes_from_path(&self, path: &Path) -> Option<&[Element<'a>]> {
        self.paths.get(path).map(|v| v.as_slice())
    }

    pub fn path_from_label(&self, label: &str) -> Option<&Path> {
        self.labels.get(label).map(|p| p.as_path())
    }
}
