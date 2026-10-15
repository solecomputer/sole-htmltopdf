use std::rc::Rc;

use std::cell::Cell;

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{ParseOpts, parse_document};

use crate::contracts::{Doctype, Dom, Error, HtmlQuirksMode, Warning};
use crate::dom::{Limits, NON_RENDERED, Node, NodeKind, Poison, Sink};

pub(crate) fn parse(
    bytes: Vec<u8>,
    origin: &str,
    base: crate::contracts::DocumentBase,
    max_nodes: usize,
    max_depth: usize,
    encoding: Option<crate::encoding::Encoding>,
) -> Result<(Dom, Vec<Warning>), Error> {
    let source = encoding
        .unwrap_or(crate::encoding::Encoding::Utf8)
        .decode(bytes, origin)?;

    let poison = Rc::new(Cell::new(None));
    let sink = Sink::new(
        Limits {
            max_nodes,
            max_depth,
        },
        Rc::clone(&poison),
    );
    let options = ParseOpts {
        tree_builder: TreeBuilderOpts::default(),
        ..ParseOpts::default()
    };

    let mut parser = parse_document(sink, options);
    for chunk in chunks(&source) {
        parser.process(StrTendril::from(chunk));
        if poison.get().is_some() {
            break;
        }
    }
    let parsed = parser.finish();

    if let Some(poison) = parsed.poison {
        return Err(match poison {
            Poison::TooManyNodes => Error::DocumentTooComplex {
                limit: "node count",
                seen: parsed.nodes_seen,
            },
            Poison::TooDeep => Error::DocumentTooComplex {
                limit: "nesting depth",
                seen: parsed.nodes_seen,
            },
        });
    }

    let doctype = parsed.document.children.borrow().iter().find_map(|child| {
        if let NodeKind::Doctype {
            name,
            public_id,
            system_id,
        } = &child.kind
        {
            Some(Doctype {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            })
        } else {
            None
        }
    });

    let mut warnings = Vec::new();
    if parsed.recovered_errors > 0 {
        warnings.push(Warning::MarkupRecovered {
            count: parsed.recovered_errors,
            first: parsed
                .first_error
                .unwrap_or_else(|| "unspecified".to_owned()),
        });
    }

    Ok((
        Dom {
            base,
            sheets: collect_sheets(&parsed.document),
            root: std::rc::Rc::clone(&parsed.document),
            doctype,
            html_quirks: match parsed.quirks {
                html5ever::tree_builder::QuirksMode::NoQuirks => HtmlQuirksMode::NoQuirks,
                html5ever::tree_builder::QuirksMode::LimitedQuirks => HtmlQuirksMode::LimitedQuirks,
                html5ever::tree_builder::QuirksMode::Quirks => HtmlQuirksMode::Quirks,
            },
        },
        warnings,
    ))
}

const CHUNK_BYTES: usize = 16 * 1024;

fn chunks(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        let mut end = CHUNK_BYTES.min(rest.len());
        while end < rest.len() && !rest.is_char_boundary(end) {
            end += 1;
        }
        let (chunk, tail) = rest.split_at(end);
        out.push(chunk);
        rest = tail;
    }
    out
}

fn collect_sheets(root: &Rc<Node>) -> Vec<crate::contracts::SheetRef> {
    use crate::contracts::SheetRef;

    let mut sheets = Vec::new();
    let mut stack: Vec<Rc<Node>> = vec![Rc::clone(root)];

    while let Some(node) = stack.pop() {
        if node.tag() == Some("link") {
            let is_stylesheet = node.attr("rel").is_some_and(|rel| {
                rel.split_ascii_whitespace()
                    .any(|r| r.eq_ignore_ascii_case("stylesheet"))
            });
            if let Some(href) = node
                .attr("href")
                .filter(|h| is_stylesheet && !h.trim().is_empty())
            {
                sheets.push(SheetRef::Linked {
                    href,
                    media: node.attr("media"),
                });
            }
        }
        if node.tag() == Some("style") {
            let mut text = String::new();
            let mut inner: Vec<Rc<Node>> = node.children.borrow().iter().map(Rc::clone).collect();
            while let Some(child) = inner.pop() {
                if let NodeKind::Text(contents) = &child.kind {
                    text.push_str(&contents.borrow());
                }
            }
            if !text.trim().is_empty() {
                sheets.push(SheetRef::Inline {
                    text,
                    media: node.attr("media"),
                });
            }
            continue;
        }
        let children = node.children.borrow();
        for child in children.iter().rev() {
            stack.push(Rc::clone(child));
        }
    }
    sheets
}

pub(crate) fn flatten_text(root: &Rc<Node>) -> String {
    let mut out = String::new();
    let mut stack: Vec<Rc<Node>> = vec![Rc::clone(root)];

    while let Some(node) = stack.pop() {
        if let NodeKind::Text(contents) = &node.kind {
            out.push_str(&contents.borrow());
            continue;
        }
        if node.tag().is_some_and(|tag| NON_RENDERED.contains(&tag)) {
            continue;
        }
        let children = node.children.borrow();

        for child in children.iter().rev() {
            stack.push(Rc::clone(child));
        }
    }
    out.trim().to_owned()
}
