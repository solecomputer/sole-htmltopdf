use std::fmt;
use std::rc::Rc;

use cssparser::{CowRcStr, SourceLocation, ToCss};
use selectors::attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint};
use selectors::context::MatchingContext;
use selectors::parser::{ParseRelative, Selector, SelectorParseErrorKind};
use selectors::{Element, OpaqueElement, SelectorImpl};

use crate::dom::{Node, NodeKind};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Default)]
pub(crate) struct CssText(pub(crate) String);

impl ToCss for CssText {
    fn to_css<W: fmt::Write>(&self, dest: &mut W) -> fmt::Result {
        cssparser::serialize_identifier(&self.0, dest)
    }
}

impl<'a> From<&'a str> for CssText {
    fn from(value: &'a str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for CssText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl precomputed_hash::PrecomputedHash for CssText {
    fn precomputed_hash(&self) -> u32 {
        0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Flavour;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum NoPseudoClass {}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum NoPseudoElement {}

impl ToCss for NoPseudoClass {
    fn to_css<W: fmt::Write>(&self, _: &mut W) -> fmt::Result {
        match *self {}
    }
}

impl ToCss for NoPseudoElement {
    fn to_css<W: fmt::Write>(&self, _: &mut W) -> fmt::Result {
        match *self {}
    }
}

impl selectors::parser::NonTSPseudoClass for NoPseudoClass {
    type Impl = Flavour;

    fn is_active_or_hover(&self) -> bool {
        match *self {}
    }

    fn is_user_action_state(&self) -> bool {
        match *self {}
    }
}

impl selectors::parser::PseudoElement for NoPseudoElement {
    type Impl = Flavour;
}

impl SelectorImpl for Flavour {
    type ExtraMatchingData<'a> = ();
    type AttrValue = CssText;
    type Identifier = CssText;
    type LocalName = CssText;
    type NamespaceUrl = CssText;
    type NamespacePrefix = CssText;
    type BorrowedNamespaceUrl = CssText;
    type BorrowedLocalName = CssText;
    type NonTSPseudoClass = NoPseudoClass;
    type PseudoElement = NoPseudoElement;
}

pub(crate) struct Grammar;

impl<'i> selectors::parser::Parser<'i> for Grammar {
    type Impl = Flavour;
    type Error = SelectorParseErrorKind<'i>;

    fn parse_non_ts_pseudo_class(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<NoPseudoClass, cssparser::ParseError<'i, Self::Error>> {
        Err(
            location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
                name,
            )),
        )
    }

    fn parse_pseudo_element(
        &self,
        location: SourceLocation,
        name: CowRcStr<'i>,
    ) -> Result<NoPseudoElement, cssparser::ParseError<'i, Self::Error>> {
        Err(
            location.new_custom_error(SelectorParseErrorKind::UnsupportedPseudoClassOrElement(
                name,
            )),
        )
    }
}

pub(crate) type SelectorList = selectors::SelectorList<Flavour>;

pub(crate) fn parse_list(source: &str) -> Result<SelectorList, ()> {
    let mut input = cssparser::ParserInput::new(source);
    let mut parser = cssparser::Parser::new(&mut input);
    selectors::SelectorList::parse(&Grammar, &mut parser, ParseRelative::No).map_err(|_| ())
}

#[derive(Clone, Debug)]
pub(crate) struct El(pub(crate) Rc<Node>);

impl El {
    fn local_name(&self) -> Option<&str> {
        self.0.tag()
    }

    fn siblings(&self) -> Vec<Rc<Node>> {
        let Some(parent) = self.0.parent_node() else {
            return Vec::new();
        };
        parent
            .children
            .borrow()
            .iter()
            .filter(|child| child.tag().is_some())
            .map(Rc::clone)
            .collect()
    }

    fn index_among_siblings(&self) -> Option<usize> {
        self.siblings()
            .iter()
            .position(|sibling| Rc::ptr_eq(sibling, &self.0))
    }
}

impl Element for El {
    type Impl = Flavour;

    fn opaque(&self) -> OpaqueElement {
        OpaqueElement::new(&*self.0)
    }

    fn parent_element(&self) -> Option<Self> {
        let parent = self.0.parent_node()?;
        parent.tag().is_some().then(|| Self(parent))
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let index = self.index_among_siblings()?;
        let siblings = self.siblings();
        index
            .checked_sub(1)
            .and_then(|before| siblings.get(before).map(|node| Self(Rc::clone(node))))
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let index = self.index_among_siblings()?;
        let siblings = self.siblings();
        siblings
            .get(index.saturating_add(1))
            .map(|node| Self(Rc::clone(node)))
    }

    fn first_element_child(&self) -> Option<Self> {
        self.0
            .children
            .borrow()
            .iter()
            .find(|child| child.tag().is_some())
            .map(|child| Self(Rc::clone(child)))
    }

    fn is_html_element_in_html_document(&self) -> bool {
        true
    }

    fn has_local_name(&self, name: &CssText) -> bool {
        self.local_name() == Some(name.as_ref())
    }

    fn has_namespace(&self, _namespace: &CssText) -> bool {
        true
    }

    fn is_same_type(&self, other: &Self) -> bool {
        self.local_name() == other.local_name()
    }

    fn attr_matches(
        &self,
        _namespace: &NamespaceConstraint<&CssText>,
        local_name: &CssText,
        operation: &AttrSelectorOperation<&CssText>,
    ) -> bool {
        self.0
            .attr(local_name.as_ref())
            .is_some_and(|value| operation.eval_str(&value))
    }

    fn has_attr_in_no_namespace(&self, local_name: &CssText) -> bool {
        self.0.attr(local_name.as_ref()).is_some()
    }

    fn has_id(&self, id: &CssText, case: CaseSensitivity) -> bool {
        self.0
            .attr("id")
            .is_some_and(|value| case.eq(value.as_bytes(), id.as_ref().as_bytes()))
    }

    fn has_class(&self, name: &CssText, case: CaseSensitivity) -> bool {
        self.0.attr("class").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|class| case.eq(class.as_bytes(), name.as_ref().as_bytes()))
        })
    }

    fn is_empty(&self) -> bool {
        self.0
            .children
            .borrow()
            .iter()
            .all(|child| match &child.kind {
                NodeKind::Text(text) => text.borrow().trim().is_empty(),
                NodeKind::Comment => true,
                NodeKind::Document | NodeKind::Doctype { .. } | NodeKind::Element { .. } => false,
            })
    }

    fn is_root(&self) -> bool {
        self.0
            .parent_node()
            .is_none_or(|parent| parent.tag().is_none())
    }

    fn is_link(&self) -> bool {
        self.local_name() == Some("a") && self.0.attr("href").is_some()
    }

    fn match_non_ts_pseudo_class(
        &self,
        pseudo: &NoPseudoClass,
        _context: &mut MatchingContext<'_, Flavour>,
    ) -> bool {
        match *pseudo {}
    }

    fn match_pseudo_element(
        &self,
        pseudo: &NoPseudoElement,
        _context: &mut MatchingContext<'_, Flavour>,
    ) -> bool {
        match *pseudo {}
    }

    fn is_pseudo_element(&self) -> bool {
        false
    }

    fn parent_node_is_shadow_root(&self) -> bool {
        false
    }

    fn containing_shadow_host(&self) -> Option<Self> {
        None
    }

    fn is_html_slot_element(&self) -> bool {
        false
    }

    fn has_custom_state(&self, _state: &CssText) -> bool {
        false
    }

    fn imported_part(&self, _name: &CssText) -> Option<CssText> {
        None
    }

    fn is_part(&self, _name: &CssText) -> bool {
        false
    }

    fn apply_selector_flags(&self, _flags: selectors::matching::ElementSelectorFlags) {}

    fn add_element_unique_hashes(&self, _filter: &mut selectors::bloom::BloomFilter) -> bool {
        false
    }
}

pub(crate) fn match_specificity(list: &SelectorList, node: &Rc<Node>) -> Option<u32> {
    let element = El(Rc::clone(node));
    list.slice()
        .iter()
        .filter(|selector| matches(selector, &element))
        .map(selectors::parser::Selector::specificity)
        .max()
}

pub(crate) fn matches(selector: &Selector<Flavour>, element: &El) -> bool {
    let mut caches = selectors::context::SelectorCaches::default();
    let mut context = MatchingContext::new(
        selectors::matching::MatchingMode::Normal,
        None,
        &mut caches,
        selectors::context::QuirksMode::NoQuirks,
        selectors::context::NeedsSelectorFlags::No,
        selectors::matching::MatchingForInvalidation::No,
    );
    selectors::matching::matches_selector(selector, 0, None, element, &mut context)
}
