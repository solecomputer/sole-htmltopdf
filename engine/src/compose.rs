use crate::contracts::{
    DocumentFacts, Edge, Error, Furniture, Length, MarginBand, PageFacts, PageFrame, Pages,
    PlacedFurniture,
};

struct Ink {
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}

impl Ink {
    fn shares_space_with(&self, other: &Self) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

pub(crate) struct Refusal {
    band: &'static str,
    axis: crate::contracts::Axis,
    needed: f32,
    available: f32,
    count: usize,
    first_page: usize,
}

pub(crate) struct Templating<'a> {
    pub(crate) document: &'a DocumentFacts,
    pub(crate) bindings: &'a [(String, String)],
}

pub(crate) fn stamp(
    pages: &mut Pages,
    furniture: &Furniture,
    subdocuments: &crate::contracts::Subdocuments,
    usable: crate::contracts::UsableHeight,
    frame: &PageFrame,
    templating: &Templating<'_>,
    warnings: &mut Vec<crate::contracts::Warning>,
) -> Result<(), Error> {
    let &Templating { document, bindings } = templating;

    if furniture.is_empty() {
        return Ok(());
    }

    let total = pages.pages.len();

    let mut pages_with_overlap = 0_usize;
    let mut refusals: Vec<Refusal> = Vec::new();
    for index in 0..total {
        let facts = PageFacts::of(pages, index, document);
        let mut placed = Vec::new();
        let mut overlaps = 0_usize;

        for (band, rect, edge) in [
            (furniture.top.as_ref(), &frame.header, Edge::Top),
            (furniture.bottom.as_ref(), &frame.footer, Edge::Bottom),
        ] {
            if let Some(band) = band.filter(|band| fits(band, rect)) {
                place(
                    band,
                    subdocuments,
                    &BandContext {
                        rect,
                        edge,
                        usable,
                        facts: &facts,
                        bindings,
                    },
                    &mut placed,
                    &mut overlaps,
                    &mut refusals,
                    warnings,
                )?;
            }
        }
        if let Some(page) = pages.pages.get_mut(index) {
            page.furniture = placed;
        }
        if overlaps > 0 {
            pages_with_overlap = pages_with_overlap.saturating_add(1);
        }
    }
    report_refusals(refusals, warnings);

    for (band, rect, edge) in [
        (furniture.top.as_ref(), &frame.header, "the header"),
        (furniture.bottom.as_ref(), &frame.footer, "the footer"),
    ] {
        if let Some(band) = band
            && !fits(band, rect)
        {
            warnings.push(crate::contracts::Warning::FurnitureRefused {
                edge,
                needed: ink_height(band),
                available: rect.height.get(),
            });
        }
    }

    for name in unsupplied_facts(furniture, subdocuments, templating, document) {
        warnings.push(crate::contracts::Warning::FurnitureFactMissing { name });
    }
    if pages_with_overlap > 0 {
        warnings.push(crate::contracts::Warning::FurnitureSlotsOverlap {
            count: pages_with_overlap,
        });
    }
    Ok(())
}

fn ink_height(band: &MarginBand) -> f32 {
    let (ascent, descent) = band.face.vertical();
    let size = band.font_size.get();
    (f32::from(ascent) + f32::from(descent)) / 1000.0 * size
}

fn report_refusals(refusals: Vec<Refusal>, warnings: &mut Vec<crate::contracts::Warning>) {
    for refusal in refusals {
        let remedy = match refusal.axis {
            crate::contracts::Axis::Vertical => {
                "raise the page margin on that edge, or shorten the markup"
            }

            crate::contracts::Axis::Horizontal => {
                "widen the page, reduce the side margins, or break the markup \
                 so it can wrap"
            }
        };
        warnings.push(crate::contracts::Warning::FurnitureTooBig {
            band: refusal.band,
            axis: refusal.axis,
            needed: refusal.needed,
            available: refusal.available,
            count: refusal.count,
            first_page: refusal.first_page,
            remedy,
        });
    }
}

fn fits(band: &MarginBand, rect: &crate::contracts::FrameRect) -> bool {
    ink_height(band) <= rect.height.get()
}

struct BandContext<'a> {
    rect: &'a crate::contracts::FrameRect,
    edge: Edge,
    usable: crate::contracts::UsableHeight,
    facts: &'a PageFacts,
    bindings: &'a [(String, String)],
}

fn place(
    band: &MarginBand,
    subdocuments: &crate::contracts::Subdocuments,
    context: &BandContext<'_>,
    out: &mut Vec<PlacedFurniture>,
    overlaps: &mut usize,
    refusals: &mut Vec<Refusal>,
    warnings: &mut Vec<crate::contracts::Warning>,
) -> Result<(), Error> {
    let &BandContext {
        rect, edge, facts, ..
    } = context;
    let size = band.font_size.get();
    let line_height = size * 1.2;

    let baseline = rect.baseline_for(edge, band.placement, band.face, size, line_height);

    if let Some(rule) = band.rule {
        let y = match edge {
            Edge::Top => rect.y.get() + rect.height.get() - rule.thickness.get(),
            Edge::Bottom => rect.y.get(),
        };
        out.push(PlacedFurniture::Rule {
            x: rect.x,
            y: Length::try_new(y, "a furniture rule's top edge")?,
            width: rect.width,
            height: rule.thickness,
            colour: rule.colour,
        });
    }

    let mut spans: Vec<Ink> = Vec::new();

    for (which, content) in band.content.iter().enumerate() {
        let slot = match content {
            crate::contracts::BoxContent::Slot(slot) => slot,
            crate::contracts::BoxContent::Document(id) => {
                place_subdocument(
                    *id,
                    subdocuments,
                    context,
                    out,
                    &mut spans,
                    refusals,
                    warnings,
                )?;
                continue;
            }
        };
        let text = slot.stamp(facts);
        if text.is_empty() {
            continue;
        }
        let width = band
            .face
            .measure(&crate::encoding::to_winansi(&text).bytes, size);
        let left = rect.x.get();
        let x = match which {
            0 => left,
            1 => left + (rect.width.get() - width) / 2.0,
            _ => left + rect.width.get() - width,
        };
        spans.push(Ink {
            top: baseline - size,
            bottom: baseline,
            left: x,
            right: x + width,
        });
        out.push(PlacedFurniture::Text {
            text,
            x: Length::try_new(x, "a furniture slot's left edge")?,
            baseline: Length::try_new(baseline, "a furniture slot's baseline")?,
            font_size: band.font_size,
            face: band.face,
            positioned: None,
            colour: band.colour,
        });
    }

    for (index, first) in spans.iter().enumerate() {
        for second in spans.iter().skip(index + 1) {
            if first.shares_space_with(second) {
                *overlaps = overlaps.saturating_add(1);
            }
        }
    }

    Ok(())
}

fn unsupplied_facts(
    furniture: &Furniture,
    subdocuments: &crate::contracts::Subdocuments,
    templating: &Templating<'_>,
    document: &DocumentFacts,
) -> Vec<&'static str> {
    let mut unsupplied: Vec<&'static str> = Vec::new();
    for band in [furniture.top.as_ref(), furniture.bottom.as_ref()]
        .into_iter()
        .flatten()
    {
        let from_bands: Vec<crate::contracts::PageToken> = band
            .content
            .iter()
            .flat_map(|content| match content {
                crate::contracts::BoxContent::Slot(slot) => slot.tokens().cloned().collect(),
                crate::contracts::BoxContent::Document(id) => subdocuments
                    .get(*id)
                    .map(|tree| {
                        let mut found = Vec::new();
                        collect_tokens(&tree.root, templating.bindings, document, &mut found);
                        found
                    })
                    .unwrap_or_default(),
            })
            .collect();
        for token in &from_bands {
            use crate::contracts::PageToken as Token;
            let missing = match *token {
                Token::Title if document.title.is_none() => "the document title",
                Token::Source if document.source.is_none() => "the input's name",
                Token::Date if document.date.is_none() => "[date]",
                Token::Time if document.time.is_none() => "[time]",

                Token::Title
                | Token::Source
                | Token::Date
                | Token::Time
                | Token::Page
                | Token::Pages
                | Token::Unanswerable(_)
                | Token::Replaced(_) => continue,
            };
            if !unsupplied.contains(&missing) {
                unsupplied.push(missing);
            }
        }
    }
    unsupplied
}

fn collect_tokens(
    node: &crate::contracts::BoxNode,
    bindings: &[(String, String)],
    document: &DocumentFacts,
    found: &mut Vec<crate::contracts::PageToken>,
) {
    if let crate::contracts::InlineContent::Text(text) = &node.content {
        let named = document.source.is_some();
        let slot = crate::contracts::parse_template(text, |name| {
            crate::contracts::resolve_token(name, bindings, named).map(|(token, _)| token)
        });
        found.extend(slot.tokens().cloned());
    }
    for child in &node.children {
        collect_tokens(child, bindings, document, found);
    }
}

fn stamp_tree(
    node: &mut crate::contracts::BoxNode,
    facts: &PageFacts,
    bindings: &[(String, String)],
    band: &'static str,
    warnings: &mut Vec<crate::contracts::Warning>,
) {
    if let crate::contracts::InlineContent::Text(text) = &node.content {
        let named = facts.has_named_source();
        let slot = crate::contracts::parse_template(text, |name| {
            let (token, notice) = crate::contracts::resolve_token(name, bindings, named)?;
            if let Some(notice) = notice {
                let raised = crate::contracts::Warning::FurnitureTemplate { band, notice };
                if !warnings.contains(&raised) {
                    warnings.push(raised);
                }
            }
            Some(token)
        });
        node.content = crate::contracts::InlineContent::Text(slot.stamp(facts));
    }
    for child in &mut node.children {
        stamp_tree(child, facts, bindings, band, warnings);
    }
}

fn place_subdocument(
    id: crate::contracts::SubdocumentId,
    subdocuments: &crate::contracts::Subdocuments,
    context: &BandContext<'_>,
    out: &mut Vec<PlacedFurniture>,
    spans: &mut Vec<Ink>,
    refusals: &mut Vec<Refusal>,
    warnings: &mut Vec<crate::contracts::Warning>,
) -> Result<(), Error> {
    let &BandContext {
        rect,
        edge,
        usable,
        facts,
        ..
    } = context;
    let named = match edge {
        Edge::Top => "top",
        Edge::Bottom => "bottom",
    };
    let Some(tree) = subdocuments.get(id) else {
        return Ok(());
    };

    let mut tree = tree.clone();
    stamp_tree(&mut tree.root, facts, context.bindings, named, warnings);

    let band_measure = crate::contracts::UsableWidth::new(
        crate::contracts::PageSize::try_new(rect.width.get(), 1.0)?,
        crate::contracts::Edges::uniform(crate::contracts::Length::from_finite(0.0)),
    )?;
    let column = crate::layout::flow(tree, band_measure, band_measure, usable)?;

    let mut needed = 0.0_f32;
    let mut widest = 0.0_f32;
    for fragment in column.units.fragments() {
        needed = needed.max(fragment.y.get() + fragment.line_height.get());

        let width = fragment.positioned.as_ref().map_or_else(
            || {
                fragment.face.measure(
                    &crate::encoding::to_winansi(&fragment.text).bytes,
                    fragment.font_size.get(),
                )
            },
            |run| run.advance,
        );
        widest = widest.max(fragment.x.get() + width);
    }

    let overflow = if needed > rect.height.get() {
        Some((crate::contracts::Axis::Vertical, needed, rect.height.get()))
    } else if widest > rect.width.get() {
        Some((crate::contracts::Axis::Horizontal, widest, rect.width.get()))
    } else {
        None
    };
    if let Some((axis, needed, available)) = overflow {
        match refusals
            .iter_mut()
            .find(|seen| seen.band == named && seen.axis == axis)
        {
            Some(seen) => {
                seen.count = seen.count.saturating_add(1);

                seen.needed = seen.needed.max(needed);
            }
            None => refusals.push(Refusal {
                band: named,
                axis,
                needed,
                available,
                count: 1,
                first_page: facts.ordinal(),
            }),
        }
        return Ok(());
    }

    for fragment in column.units.fragments() {
        if fragment.text.is_empty() {
            continue;
        }

        let left = rect.x.get() + fragment.x.get();
        let width = fragment.face.measure(
            &crate::encoding::to_winansi(&fragment.text).bytes,
            fragment.font_size.get(),
        );
        let baseline = rect.y.get() + fragment.y.get();
        spans.push(Ink {
            top: baseline - fragment.font_size.get(),
            bottom: baseline,
            left,
            right: left + width,
        });
        out.push(PlacedFurniture::Text {
            text: fragment.text.clone(),
            x: crate::contracts::Length::from_finite(rect.x.get() + fragment.x.get()),
            baseline: crate::contracts::Length::from_finite(rect.y.get() + fragment.y.get()),
            font_size: fragment.font_size,
            face: fragment.face,
            positioned: fragment.positioned.clone(),
            colour: fragment.colour,
        });
    }
    Ok(())
}
