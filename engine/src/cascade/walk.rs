use super::{
    ComputedStyle, Diagnostics, NON_RENDERED, Node, NodeKind, SpacingEdges, SpecifiedEdges,
    SpecifiedPaint, StyledNode, UNIMPLEMENTED_ATTRIBUTES, apply, css, ordered_declarations,
};

pub(super) struct Walk<'a> {
    rules: &'a [css::Rule],
    diagnostics: &'a mut Diagnostics,
    colours: crate::contracts::Colours,
    link_policy: crate::contracts::LinkPolicy,
    link_refusals: &'a mut crate::contracts::LinkRefusals,
}

impl<'a> Walk<'a> {
    pub(super) fn new(
        rules: &'a [css::Rule],
        diagnostics: &'a mut Diagnostics,
        colours: crate::contracts::Colours,
        link_policy: crate::contracts::LinkPolicy,
        link_refusals: &'a mut crate::contracts::LinkRefusals,
    ) -> Self {
        Self {
            rules,
            diagnostics,
            colours,
            link_policy,
            link_refusals,
        }
    }

    pub(super) fn style_node(
        &mut self,
        node: &std::rc::Rc<Node>,
        inherited: &ComputedStyle,
        active_link: Option<crate::contracts::LinkTarget>,
    ) -> Option<StyledNode> {
        let mut style = ComputedStyle {
            font_size: inherited.font_size,

            border: crate::contracts::BorderEdges::default(),

            text_align: inherited.text_align,

            line_height: inherited.line_height,

            colour: inherited.colour,

            surface: inherited.surface,
            paints: inherited.paints,

            font_family: inherited.font_family,
            font_families: inherited.font_families.clone(),
            font_weight: inherited.font_weight,
            font_style: inherited.font_style,

            margin: SpacingEdges::default(),

            padding: SpacingEdges::default(),
            width: crate::contracts::Width::Auto,
            box_sizing: crate::contracts::BoxSizing::ContentBox,

            display: crate::contracts::Display::Inline,
            flex_container: crate::contracts::FlexContainerStyle::default(),
            flex_item: crate::contracts::FlexItemStyle::default(),
        };
        let mut hidden = false;

        let mut borders = SpecifiedEdges::default();
        let mut paint = SpecifiedPaint::default();

        let inline = node
            .attr("style")
            .map(|list| css::parse_declarations(&list));
        if let Some(parsed) = &inline {
            self.diagnostics.absorb(parsed);
        }

        for declared in ordered_declarations(node, self.rules, inline.as_ref()) {
            apply(
                declared.property.clone(),
                &mut style,
                inherited.font_size,
                &mut borders,
                &mut paint,
                &mut hidden,
                &mut self.diagnostics.out_of_range,
            );
        }

        let declared_foreground = paint.foreground.is_some();
        let painted = paint.promote(
            inherited.surface,
            inherited.colour,
            inherited.paints,
            style.display,
            self.colours,
        );
        style.colour = painted.colour;
        style.surface = painted.surface;
        style.paints = painted.paints;
        if let Some(what) = painted.refusal {
            self.diagnostics
                .translated
                .record(&paired_refusal(&what, declared_foreground));
        }
        style.border = borders.promote(style.font_size, style.colour);

        if hidden {
            return None;
        }

        let link = self.admitted_link(node, active_link);

        let image = read_image_request(node);
        report_unimplemented_attributes(node, image.is_some(), self.diagnostics);

        let tag = node.tag().map(str::to_owned);
        let text = match &node.kind {
            NodeKind::Text(contents) => Some(contents.take()),
            NodeKind::Document
            | NodeKind::Doctype { .. }
            | NodeKind::Element { .. }
            | NodeKind::Comment => None,
        };

        if style.display == crate::contracts::Display::Inline {
            let spacing_declared = |edges: &SpacingEdges| {
                [edges.left, edges.right, edges.top, edges.bottom]
                    .iter()
                    .any(|side| *side != crate::contracts::Spacing::ZERO)
            };
            if spacing_declared(&style.margin) {
                self.diagnostics
                    .unsupported
                    .record("margin on an inline element");
            }
            if spacing_declared(&style.padding) {
                self.diagnostics
                    .unsupported
                    .record("padding on an inline element");
            }
        }

        let children = self.style_children(node, &style, link.as_ref());

        Some(StyledNode {
            tag,
            text,
            style,
            image,
            link,
            children,
        })
    }

    fn style_children(
        &mut self,
        node: &std::rc::Rc<Node>,
        style: &ComputedStyle,
        link: Option<&crate::contracts::LinkTarget>,
    ) -> Vec<StyledNode> {
        node.children
            .borrow()
            .iter()
            .filter(|child| !child.tag().is_some_and(|tag| NON_RENDERED.contains(&tag)))
            .filter_map(|child| self.style_node(child, style, link.cloned()))
            .collect()
    }

    fn admitted_link(
        &mut self,
        node: &std::rc::Rc<Node>,
        active: Option<crate::contracts::LinkTarget>,
    ) -> Option<crate::contracts::LinkTarget> {
        if active.is_some() || node.tag() != Some("a") {
            return active;
        }
        let href = node.attr("href")?;
        crate::contracts::LinkTarget::validate(&href, self.link_policy, self.link_refusals)
    }
}

fn report_unimplemented_attributes(
    node: &std::rc::Rc<Node>,
    is_image: bool,
    diagnostics: &mut Diagnostics,
) {
    for name in UNIMPLEMENTED_ATTRIBUTES {
        if is_image && matches!(*name, "width" | "height") && honoured_hint(node, name) {
            continue;
        }
        if node.attr(name).is_some() {
            diagnostics.html_attributes.record(name);
        }
    }
}

fn honoured_hint(node: &std::rc::Rc<Node>, name: &str) -> bool {
    node.attr(name).is_none_or(|raw| parse_hint(&raw).is_some())
}

fn parse_hint(raw: &str) -> Option<f32> {
    let digits = raw.trim();
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    digits.parse::<u16>().ok().map(f32::from)
}

fn read_image_request(node: &crate::dom::Node) -> Option<crate::contracts::ImageRequest> {
    if node.tag().is_none_or(|tag| tag != "img") {
        return None;
    }
    let hint = |name: &str| -> Option<f32> { parse_hint(&node.attr(name)?) };
    Some(crate::contracts::ImageRequest {
        source: node.attr("src").unwrap_or_default(),
        alt: node.attr("alt"),
        width: hint("width"),
        height: hint("height"),
    })
}

fn paired_refusal(what: &str, declared_foreground: bool) -> String {
    if declared_foreground {
        format!("{what}; the element's own colours were not applied")
    } else {
        format!("{what}; it was not painted")
    }
}
