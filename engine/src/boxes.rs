use crate::contracts::{BoxNode, BoxTree, StyledNode, StyledTree};

pub(crate) fn build_with_fonts(
    styled: StyledTree,
    warnings: &mut Vec<crate::contracts::Warning>,
    fonts: Option<&crate::font::FontTable>,
) -> Result<BoxTree, crate::contracts::Error> {
    let mut inline_borders = 0_usize;
    let mut shared_lines = 0_usize;
    let mut refused_text: Vec<(&'static str, String)> = Vec::new();
    let canvas = canvas_of(&styled.root);
    let mut styled = styled;
    refuse_images_sharing_a_line(&mut styled.root, &mut shared_lines);
    let root = build_node(styled.root, &mut inline_borders, fonts, &mut refused_text)?;

    if shared_lines > 0 {
        warnings.push(crate::contracts::Warning::ImageSharesItsLine {
            count: shared_lines,
        });
    }

    if inline_borders > 0 {
        warnings.push(crate::contracts::Warning::BorderOnInlineBox {
            count: inline_borders,
        });
    }
    for reason in [
        "it contains an explicit bidi-formatting control",
        "right-to-left or mixed-direction shaping is outside this bounded slice",
        "its script is outside Latin, Greek, Cyrillic, Common and Inherited",
    ] {
        let matching = refused_text
            .iter()
            .filter(|(observed, _)| *observed == reason)
            .collect::<Vec<_>>();
        if let Some((_, first)) = matching.first() {
            warnings.push(crate::contracts::Warning::AuthoredTextRefused {
                count: matching.len(),
                reason,
                first: first.chars().take(80).collect(),
            });
        }
    }
    Ok(BoxTree { root, canvas })
}

fn refuse_images_sharing_a_line(node: &mut StyledNode, refused: &mut usize) {
    if inline_contributors(node) > 1 {
        refuse_images_below(node, refused);
    }
    for child in &mut node.children {
        refuse_images_sharing_a_line(child, refused);
    }
}

fn inline_contributors(node: &StyledNode) -> usize {
    node.children
        .iter()
        .filter(|child| child.style.display == crate::contracts::Display::Inline)
        .fold(0, |total, child| {
            total.saturating_add(contributions_of(child))
        })
}

fn contributions_of(node: &StyledNode) -> usize {
    let own = usize::from(node.image.is_some())
        + usize::from(node.tag.as_deref() == Some("br"))
        + usize::from(
            node.text
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty()),
        );
    node.children
        .iter()
        .filter(|child| child.style.display == crate::contracts::Display::Inline)
        .fold(own, |total, child| {
            total.saturating_add(contributions_of(child))
        })
}

fn refuse_images_below(node: &mut StyledNode, refused: &mut usize) {
    for child in &mut node.children {
        if child.style.display != crate::contracts::Display::Inline {
            continue;
        }
        if let Some(request) = child.image.take() {
            *refused = refused.saturating_add(1);

            child.text = request.alt.filter(|alt| !alt.is_empty());
        }
        refuse_images_below(child, refused);
    }
}

fn canvas_of(root: &crate::contracts::StyledNode) -> Option<crate::contracts::Rgb> {
    fn surface(node: &crate::contracts::StyledNode) -> Option<crate::contracts::Rgb> {
        match node.style.surface {
            crate::contracts::Surface::Canvas => None,
            crate::contracts::Surface::Colour(colour) => Some(colour),
        }
    }
    fn named<'tree>(
        node: &'tree crate::contracts::StyledNode,
        tag: &str,
    ) -> Option<&'tree crate::contracts::StyledNode> {
        node.children
            .iter()
            .find(|child| child.tag.as_deref() == Some(tag))
    }

    let html = named(root, "html").unwrap_or(root);
    surface(html).or_else(|| named(html, "body").and_then(surface))
}

fn build_node(
    mut node: StyledNode,
    inline_borders: &mut usize,
    fonts: Option<&crate::font::FontTable>,
    refused_text: &mut Vec<(&'static str, String)>,
) -> Result<BoxNode, crate::contracts::Error> {
    let font_size = node.style.font_size;
    let flex_container = crate::contracts::BoxFlexContainer {
        direction: node.style.flex_container.direction,
        justify: node.style.flex_container.justify,
        align_items: node.style.flex_container.align_items,
        row_gap: node.style.flex_container.row_gap.resolve(font_size)?,
        column_gap: node.style.flex_container.column_gap.resolve(font_size)?,
    };
    let flex_item = crate::contracts::BoxFlexItem {
        grow: node.style.flex_item.grow,
        shrink: node.style.flex_item.shrink,
        basis: match node.style.flex_item.basis {
            crate::contracts::FlexBasis::Auto => crate::contracts::BoxFlexBasis::Auto,
            crate::contracts::FlexBasis::Content => crate::contracts::BoxFlexBasis::Content,
            crate::contracts::FlexBasis::Fixed(value) => {
                crate::contracts::BoxFlexBasis::Fixed(value.resolve(font_size)?)
            }
            crate::contracts::FlexBasis::Percent(value) => {
                crate::contracts::BoxFlexBasis::Percent(value)
            }
        },
        align_self: node.style.flex_item.align_self,
    };

    let inline = node.style.display == crate::contracts::Display::Inline;
    let border = if inline && node.style.border.any() {
        *inline_borders = inline_borders.saturating_add(1);
        crate::contracts::BorderEdges::default()
    } else {
        node.style.border
    };
    let content = match (node.tag.as_deref(), node.text.take(), node.image.take()) {
        (Some("br"), _, _) => crate::contracts::InlineContent::ForcedBreak,
        (_, _, Some(request)) => crate::contracts::InlineContent::Replaced(Box::new(request)),
        (_, Some(text), _) => crate::contracts::InlineContent::Text(text),
        (_, None, _) => crate::contracts::InlineContent::Empty,
    };
    let authored_face = if let crate::contracts::InlineContent::Text(text) = &content {
        if let Err(reason) = crate::font::text_admission(text) {
            refused_text.push((reason, text.clone()));
            None
        } else {
            fonts.and_then(|fonts| {
                fonts.select_for_text(
                    &node.style.font_families,
                    node.style.font_weight,
                    node.style.font_style,
                    text,
                )
            })
        }
    } else {
        fonts.and_then(|fonts| {
            fonts.select(
                &node.style.font_families,
                node.style.font_weight,
                node.style.font_style,
            )
        })
    };
    Ok(BoxNode {
        link: node.link,

        content,
        font_size,
        margin: node.style.margin.resolve(font_size)?,
        padding: node.style.padding.resolve(font_size)?,

        border,
        line_height: node.style.line_height.resolve(font_size)?,

        width: node.style.width,
        box_sizing: node.style.box_sizing,
        display: node.style.display,
        flex_container,
        flex_item,
        text_align: node.style.text_align,

        face: crate::metrics::Face::resolve(
            node.style.font_family,
            node.style.font_weight,
            node.style.font_style,
        ),
        authored_face,

        colour: node.style.colour,
        surface: node.style.surface,
        paints: node.style.paints,
        children: node
            .children
            .into_iter()
            .map(|child| build_node(child, inline_borders, fonts, refused_text))
            .collect::<Result<Vec<_>, _>>()?,
    })
}
