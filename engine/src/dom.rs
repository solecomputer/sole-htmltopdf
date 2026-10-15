use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use html5ever::tendril::StrTendril;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{Attribute, ExpandedName, QualName};

#[derive(Debug)]
pub(crate) enum NodeKind {
    Document,

    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        name: QualName,
        attrs: RefCell<Vec<(String, String)>>,
    },
    Text(RefCell<String>),
    Comment,
}

#[derive(Debug)]
pub(crate) struct Node {
    pub(crate) kind: NodeKind,
    pub(crate) children: RefCell<Vec<Rc<Node>>>,

    pub(crate) depth: Cell<usize>,

    parent: RefCell<Weak<Node>>,
}

impl Node {
    fn new(kind: NodeKind) -> Rc<Self> {
        Rc::new(Self {
            kind,
            children: RefCell::new(Vec::new()),
            depth: Cell::new(0),
            parent: RefCell::new(Weak::new()),
        })
    }

    pub(crate) fn parent_node(&self) -> Option<Rc<Node>> {
        self.parent.borrow().upgrade()
    }

    fn parent(&self) -> Option<Rc<Node>> {
        self.parent.borrow().upgrade()
    }

    fn adopt(parent: &Rc<Node>, child: &Rc<Node>, index: usize) {
        *child.parent.borrow_mut() = Rc::downgrade(parent);
        Self::redepth(child, parent.depth.get().saturating_add(1));
        let mut children = parent.children.borrow_mut();
        let index = index.min(children.len());
        children.insert(index, Rc::clone(child));
    }

    fn redepth(root: &Rc<Node>, depth: usize) {
        let mut stack = vec![(Rc::clone(root), depth)];
        while let Some((node, depth)) = stack.pop() {
            node.depth.set(depth);
            let next = depth.saturating_add(1);
            for child in node.children.borrow().iter() {
                stack.push((Rc::clone(child), next));
            }
        }
    }

    fn deepest(root: &Rc<Node>) -> usize {
        let mut deepest = root.depth.get();
        let mut stack = vec![Rc::clone(root)];
        while let Some(node) = stack.pop() {
            deepest = deepest.max(node.depth.get());
            for child in node.children.borrow().iter() {
                stack.push(Rc::clone(child));
            }
        }
        deepest
    }

    fn detach(node: &Rc<Node>) -> Option<usize> {
        let parent = node.parent()?;
        let mut children = parent.children.borrow_mut();
        let index = children.iter().position(|child| Rc::ptr_eq(child, node))?;
        children.remove(index);
        drop(children);
        *node.parent.borrow_mut() = Weak::new();
        Some(index)
    }

    pub(crate) fn attr(&self, wanted: &str) -> Option<String> {
        match &self.kind {
            NodeKind::Element { attrs, .. } => attrs
                .borrow()
                .iter()
                .find(|(name, _)| name == wanted)
                .map(|(_, value)| value.clone()),
            NodeKind::Document
            | NodeKind::Doctype { .. }
            | NodeKind::Text(_)
            | NodeKind::Comment => None,
        }
    }

    pub(crate) fn tag(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Element { name, .. } => Some(&name.local),
            NodeKind::Document
            | NodeKind::Doctype { .. }
            | NodeKind::Text(_)
            | NodeKind::Comment => None,
        }
    }
}

pub(crate) const NON_RENDERED: [&str; 5] = ["head", "style", "script", "template", "title"];

#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) max_nodes: usize,
    pub(crate) max_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Poison {
    TooManyNodes,
    TooDeep,
}

pub(crate) struct Parsed {
    pub(crate) document: Rc<Node>,

    pub(crate) nodes_seen: usize,
    pub(crate) quirks: QuirksMode,
    pub(crate) recovered_errors: usize,
    pub(crate) first_error: Option<String>,
    pub(crate) poison: Option<Poison>,
}

pub(crate) struct Sink {
    document: Rc<Node>,
    limits: Limits,
    nodes: Cell<usize>,

    poison: Rc<Cell<Option<Poison>>>,
    quirks: Cell<QuirksMode>,
    errors: Cell<usize>,

    saw_doctype: Cell<bool>,
    first_error: RefCell<Option<String>>,

    fallback_name: QualName,
}

impl Sink {
    pub(crate) fn new(limits: Limits, poison: Rc<Cell<Option<Poison>>>) -> Self {
        Self {
            document: Node::new(NodeKind::Document),
            limits,
            nodes: Cell::new(1),
            poison,
            quirks: Cell::new(QuirksMode::NoQuirks),
            errors: Cell::new(0),
            saw_doctype: Cell::new(false),
            first_error: RefCell::new(None),
            fallback_name: QualName::new(None, html5ever::ns!(html), html5ever::local_name!("div")),
        }
    }

    fn admit_node(&self) -> bool {
        let next = self.nodes.get().saturating_add(1);
        self.nodes.set(next);
        if self.poison.get().is_some() {
            return false;
        }
        if next > self.limits.max_nodes {
            self.poison.set(Some(Poison::TooManyNodes));
            return false;
        }
        true
    }
}

impl TreeSink for Sink {
    type Handle = Rc<Node>;
    type Output = Parsed;
    type ElemName<'a>
        = ExpandedName<'a>
    where
        Self: 'a;

    fn finish(self) -> Parsed {
        Parsed {
            document: self.document,
            nodes_seen: self.nodes.get(),
            quirks: self.quirks.get(),
            recovered_errors: self.errors.get(),
            first_error: self.first_error.into_inner(),
            poison: self.poison.get(),
        }
    }

    fn parse_error(&self, message: std::borrow::Cow<'static, str>) {
        if !self.saw_doctype.get() && self.errors.get() == 0 && message.contains("Unexpected token")
        {
            self.saw_doctype.set(true);
            return;
        }

        self.errors.set(self.errors.get().saturating_add(1));
        let mut first = self.first_error.borrow_mut();
        if first.is_none() {
            *first = Some(message.into_owned());
        }
    }

    fn get_document(&self) -> Rc<Node> {
        Rc::clone(&self.document)
    }

    fn elem_name<'a>(&'a self, target: &'a Rc<Node>) -> ExpandedName<'a> {
        match &target.kind {
            NodeKind::Element { name, .. } => name.expanded(),

            NodeKind::Document
            | NodeKind::Doctype { .. }
            | NodeKind::Text(_)
            | NodeKind::Comment => self.fallback_name.expanded(),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, _: ElementFlags) -> Rc<Node> {
        let admitted = self.admit_node();
        let pairs = if admitted {
            attrs
                .into_iter()
                .map(|a| (a.name.local.to_string(), a.value.to_string()))
                .collect()
        } else {
            Vec::new()
        };
        Node::new(NodeKind::Element {
            name,
            attrs: RefCell::new(pairs),
        })
    }

    fn create_comment(&self, _: StrTendril) -> Rc<Node> {
        let _ = self.admit_node();
        Node::new(NodeKind::Comment)
    }

    fn create_pi(&self, _: StrTendril, _: StrTendril) -> Rc<Node> {
        let _ = self.admit_node();
        Node::new(NodeKind::Comment)
    }

    fn append(&self, parent: &Rc<Node>, child: NodeOrText<Rc<Node>>) {
        if self.poison.get().is_some() {
            return;
        }
        let depth = parent.depth.get().saturating_add(1);
        if depth > self.limits.max_depth {
            self.poison.set(Some(Poison::TooDeep));
            return;
        }

        let node = match child {
            NodeOrText::AppendNode(node) => {
                let _ = Node::detach(&node);
                node
            }
            NodeOrText::AppendText(text) => {
                if let Some(NodeKind::Text(existing)) =
                    parent.children.borrow().last().map(|last| &last.kind)
                {
                    existing.borrow_mut().push_str(&text);
                    return;
                }
                if !self.admit_node() {
                    return;
                }
                Node::new(NodeKind::Text(RefCell::new(text.to_string())))
            }
        };
        let index = parent.children.borrow().len();
        Node::adopt(parent, &node, index);

        if Node::deepest(&node) > self.limits.max_depth {
            self.poison.set(Some(Poison::TooDeep));
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Rc<Node>,
        _: &Rc<Node>,
        child: NodeOrText<Rc<Node>>,
    ) {
        self.append(element, child);
    }

    fn append_doctype_to_document(&self, name: StrTendril, public: StrTendril, system: StrTendril) {
        if !self.admit_node() {
            return;
        }
        self.saw_doctype.set(true);

        let node = Node::new(NodeKind::Doctype {
            name: name.to_string(),
            public_id: public.to_string(),
            system_id: system.to_string(),
        });
        let index = self.document.children.borrow().len();
        Node::adopt(&self.document, &node, index);
    }

    fn get_template_contents(&self, target: &Rc<Node>) -> Rc<Node> {
        Rc::clone(target)
    }

    fn same_node(&self, a: &Rc<Node>, b: &Rc<Node>) -> bool {
        Rc::ptr_eq(a, b)
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.quirks.set(mode);
    }

    fn append_before_sibling(&self, sibling: &Rc<Node>, child: NodeOrText<Rc<Node>>) {
        let Some(parent) = sibling.parent() else {
            self.append(sibling, child);
            return;
        };
        if self.poison.get().is_some() {
            return;
        }

        let node = match child {
            NodeOrText::AppendNode(node) => {
                let _ = Node::detach(&node);
                node
            }
            NodeOrText::AppendText(text) => {
                if !self.admit_node() {
                    return;
                }
                Node::new(NodeKind::Text(RefCell::new(text.to_string())))
            }
        };

        let index = parent
            .children
            .borrow()
            .iter()
            .position(|existing| Rc::ptr_eq(existing, sibling))
            .unwrap_or(0);
        Node::adopt(&parent, &node, index);

        if Node::deepest(&node) > self.limits.max_depth {
            self.poison.set(Some(Poison::TooDeep));
        }
    }

    fn add_attrs_if_missing(&self, target: &Rc<Node>, attrs: Vec<Attribute>) {
        if let NodeKind::Element {
            attrs: existing, ..
        } = &target.kind
        {
            let mut existing = existing.borrow_mut();
            for attr in attrs {
                let name = attr.name.local.to_string();
                if !existing.iter().any(|(have, _)| *have == name) {
                    existing.push((name, attr.value.to_string()));
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &Rc<Node>) {
        let _ = Node::detach(target);
    }

    fn reparent_children(&self, from: &Rc<Node>, to: &Rc<Node>) {
        let moved: Vec<Rc<Node>> = from.children.borrow_mut().drain(..).collect();
        for node in moved {
            *node.parent.borrow_mut() = Weak::new();
            let index = to.children.borrow().len();
            Node::adopt(to, &node, index);
            if Node::deepest(&node) > self.limits.max_depth {
                self.poison.set(Some(Poison::TooDeep));
            }
        }
    }
}
