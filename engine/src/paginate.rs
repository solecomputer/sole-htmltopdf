use crate::contracts::{
    BorderSide, Edges, Error, FlowUnit, FragmentTree, Length, Page, PageSize, Pages,
    PlacedFragment, PlacedRect, Warning,
};

const MAX_PAGES: usize = 100_000;

struct PlacedPage {
    bands: Vec<PlacedRect>,
    fragments: Vec<PlacedFragment>,
    images: Vec<crate::contracts::PlacedPicture>,
    links: Vec<crate::contracts::PlacedLink>,
}

fn place_page(
    units: Vec<FlowUnit>,
    page_top: f32,
    top: f32,
    usable_height: f32,
    carried: &mut Vec<PlacedRect>,
    carried_text: &mut Vec<PlacedFragment>,
    carried_links: &mut Vec<crate::contracts::PlacedLink>,
) -> Result<PlacedPage, Error> {
    let mut page = PlacedPage {
        bands: Vec::new(),
        fragments: Vec::new(),
        images: Vec::new(),
        links: Vec::new(),
    };

    resume_carried(carried, &mut page.bands, top, usable_height)?;
    resume_carried_links(carried_links, &mut page.links, top, usable_height)?;

    for piece in std::mem::take(carried_text) {
        page.fragments.push(piece);
    }
    for unit in units {
        let placed = PlacedUnit::place(unit, page_top, top)?;
        let (mut bands, mut fragments, mut images, mut links) = placed.into_parts();

        clip_to_page(&mut bands, carried, top + usable_height)?;
        clip_links_to_page(&mut links, carried_links, top, usable_height)?;
        bands.retain(|band| band.height.get() > 0.0);
        links.retain(|link| link.height.get() > 0.0);
        page.bands.append(&mut bands);
        page.fragments.append(&mut fragments);
        page.images.append(&mut images);
        page.links.append(&mut links);
    }
    carry_overflowing_glyphs(&mut page.fragments, carried_text, top, usable_height)?;
    Ok(page)
}

fn carry_overflowing_glyphs(
    fragments: &mut [PlacedFragment],
    carried_text: &mut Vec<PlacedFragment>,
    top: f32,
    usable_height: f32,
) -> Result<(), Error> {
    let floor = top + usable_height;
    let band = crate::contracts::PageBand {
        top: Length::try_new(top, "a page band's top")?,
        height: Length::try_new(usable_height, "a page band's height")?,
    };
    for fragment in fragments.iter_mut() {
        let (above, below) = fragment.positioned.as_ref().map_or_else(
            || fragment.face.ink_extent(fragment.font_size.get()),
            |run| (run.ink_above, run.ink_below),
        );
        let ink_bottom = fragment.y.get() + below;
        let _ = above;

        if ink_bottom <= floor {
            continue;
        }
        fragment.clip = Some(band);
        if ink_bottom > floor {
            carried_text.push(PlacedFragment {
                positioned: fragment.positioned.clone(),

                y: Length::try_new(
                    fragment.y.get() - usable_height,
                    "a continued glyph's baseline",
                )?,
                clip: Some(band),
                continued: true,
                text: fragment.text.clone(),
                ..*fragment
            });
        }
    }
    Ok(())
}

fn attach_decorations(
    decorations: &[crate::layout::Decoration],
    pages: &mut [Page],
    page_anchor: &[f32],
    pieces: &crate::contracts::PieceMap,
    top: f32,
    usable_height: f32,
    warnings: &mut Vec<Warning>,
) -> Result<(), Error> {
    let mut fragmented = 0_usize;

    let mut leaves_page = 0_usize;
    for decoration in decorations {
        let mut home: Option<usize> = None;
        let mut spans = false;
        for member in decoration.members() {
            let Some(page) = pieces.sole_page(*member) else {
                spans = true;
                break;
            };
            match home {
                None => home = Some(page),
                Some(first) if first == page => {}
                Some(_) => {
                    spans = true;
                    break;
                }
            }
        }
        match (spans, home) {
            (false, Some(page))
                if fits_in(decoration, page_anchor.get(page).copied(), usable_height) =>
            {
                if let (Some(target), Some(anchor)) =
                    (pages.get_mut(page), page_anchor.get(page).copied())
                {
                    push_sides(&mut target.decorations, decoration, anchor, top)?;
                }
            }

            (false, Some(_)) => leaves_page = leaves_page.saturating_add(1),

            (true, Some(_)) => {
                let mut reached: Vec<usize> = Vec::new();
                for member in decoration.members() {
                    for piece in pieces.pieces(*member) {
                        if !reached.contains(&piece.page()) {
                            reached.push(piece.page());
                        }
                    }
                }
                let (Some(first), Some(last)) =
                    (reached.iter().min().copied(), reached.iter().max().copied())
                else {
                    fragmented = fragmented.saturating_add(1);
                    continue;
                };
                for page in first..=last {
                    if let (Some(target), Some(anchor)) =
                        (pages.get_mut(page), page_anchor.get(page).copied())
                    {
                        push_fragment_sides(
                            &mut target.decorations,
                            decoration,
                            anchor,
                            top,
                            usable_height,
                        )?;
                    }
                }
            }
            _ => fragmented = fragmented.saturating_add(1),
        }
    }
    if fragmented > 0 {
        warnings.push(Warning::BorderOnFragmentedBlock { count: fragmented });
    }
    if leaves_page > 0 {
        warnings.push(Warning::BorderLeavesItsPage { count: leaves_page });
    }
    Ok(())
}

fn report_layout_losses(
    not_enclosing: usize,
    on_rows: usize,
    trimmed: (usize, f32),
    warnings: &mut Vec<Warning>,
) {
    if trimmed.0 > 0 {
        warnings.push(Warning::LeadingTrimmedToFit {
            count: trimmed.0,
            points: trimmed.1,
        });
    }
    if not_enclosing > 0 {
        warnings.push(Warning::BorderDoesNotEncloseItsContent {
            count: not_enclosing,
        });
    }
    if on_rows > 0 {
        warnings.push(Warning::BorderOnRowOrRowGroup { count: on_rows });
    }
}

fn fits_in(
    decoration: &crate::layout::Decoration,
    anchor: Option<f32>,
    usable_height: f32,
) -> bool {
    anchor.is_some_and(|anchor| {
        let bottom = decoration.top().get() + decoration.height().get();
        decoration.top().get() >= anchor && bottom <= anchor + usable_height + EDGE_TOLERANCE
    })
}

const EDGE_TOLERANCE: f32 = 0.01;

fn push_sides(
    out: &mut Vec<PlacedRect>,
    decoration: &crate::layout::Decoration,
    page_top: f32,
    top: f32,
) -> Result<(), Error> {
    let local = |value: f32| Length::try_new(value - page_top + top, "placing a decoration");
    let border = decoration.border();
    let (left, width) = (decoration.left().get(), decoration.width().get());
    let (box_top, height) = (decoration.top().get(), decoration.height().get());

    let mut side = |x: f32, y: f32, w: f32, h: f32, colour| -> Result<(), Error> {
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }
        out.push(PlacedRect {
            x: Length::try_new(x, "a decoration's x")?,
            y: local(y)?,
            width: Length::try_new(w, "a decoration's width")?,
            height: Length::try_new(h, "a decoration's height")?,
            colour,
        });
        Ok(())
    };

    if let BorderSide::Solid { width: w, colour } = border.top {
        side(left, box_top, width, w.get(), colour)?;
    }
    if let BorderSide::Solid { width: w, colour } = border.bottom {
        side(left, box_top + height - w.get(), width, w.get(), colour)?;
    }
    if let BorderSide::Solid { width: w, colour } = border.left {
        side(left, box_top, w.get(), height, colour)?;
    }
    if let BorderSide::Solid { width: w, colour } = border.right {
        side(left + width - w.get(), box_top, w.get(), height, colour)?;
    }
    Ok(())
}

fn push_fragment_sides(
    out: &mut Vec<PlacedRect>,
    decoration: &crate::layout::Decoration,
    page_top: f32,
    top: f32,
    usable_height: f32,
) -> Result<(), Error> {
    let local = |value: f32| Length::try_new(value - page_top + top, "placing a decoration piece");
    let border = decoration.border();
    let (left, width) = (decoration.left().get(), decoration.width().get());
    let (box_top, height) = (decoration.top().get(), decoration.height().get());
    let (box_bottom, page_floor) = (box_top + height, page_top + usable_height);

    let piece_top = box_top.max(page_top);
    let piece_bottom = box_bottom.min(page_floor);
    if piece_bottom <= piece_top {
        return Ok(());
    }

    let mut side = |x: f32, y: f32, w: f32, h: f32, colour| -> Result<(), Error> {
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }
        out.push(PlacedRect {
            x: Length::try_new(x, "a decoration piece's x")?,
            y: local(y)?,
            width: Length::try_new(w, "a decoration piece's width")?,
            height: Length::try_new(h, "a decoration piece's height")?,
            colour,
        });
        Ok(())
    };

    if let BorderSide::Solid { width: w, colour } = border.top
        && box_top >= page_top
    {
        side(left, box_top, width, w.get(), colour)?;
    }
    if let BorderSide::Solid { width: w, colour } = border.bottom
        && box_bottom <= page_floor
    {
        side(left, box_bottom - w.get(), width, w.get(), colour)?;
    }
    let extent = piece_bottom - piece_top;
    if let BorderSide::Solid { width: w, colour } = border.left {
        side(left, piece_top, w.get(), extent, colour)?;
    }
    if let BorderSide::Solid { width: w, colour } = border.right {
        side(left + width - w.get(), piece_top, w.get(), extent, colour)?;
    }
    Ok(())
}

pub(crate) fn paginate(
    tree: FragmentTree,
    page_size: PageSize,
    page_margin: Edges,
    usable: crate::contracts::UsableHeight,
    warnings: &mut Vec<Warning>,
) -> Result<Pages, Error> {
    let top = page_margin.top.get();

    let usable_height = usable.get();
    let content_height = tree.content_height.get();
    let canvas = tree.canvas;

    let box_top = FlowUnit::top;
    let box_bottom = |unit: &FlowUnit| unit.top().get() + unit.height().get();

    let mut piece_map = crate::contracts::PieceMap::for_units(tree.units.len());
    let mut next_unit = 0_usize;

    let mut remaining = tree.units;
    let mut pages: Vec<Page> = Vec::new();

    let mut page_anchor: Vec<f32> = Vec::new();
    let mut page_top = 0.0_f32;

    let mut carried: Vec<PlacedRect> = Vec::new();

    let mut carried_text: Vec<PlacedFragment> = Vec::new();
    let mut carried_links: Vec<crate::contracts::PlacedLink> = Vec::new();

    loop {
        let page_bottom = page_top + usable_height;

        let mut taken = remaining
            .as_slice()
            .iter()
            .take_while(|unit| box_bottom(unit) <= page_bottom + f32::EPSILON)
            .count();

        if taken == 0
            && let Some(unit) = remaining.as_slice().first()
            && unit.height().get() > usable_height
        {
            taken = 1;
        }

        let mine = remaining.take_first(taken);

        let page_index = pages.len();
        for offset in 0..mine.len() {
            piece_map.record(
                next_unit + offset,
                crate::contracts::PagePiece::reserving(page_index),
            );
        }
        next_unit = next_unit.saturating_add(mine.len());

        let placed = place_page(
            mine,
            page_top,
            top,
            usable_height,
            &mut carried,
            &mut carried_text,
            &mut carried_links,
        )?;

        page_anchor.push(page_top);
        pages.push(Page {
            furniture: Vec::new(),
            fragments: placed.fragments,
            images: placed.images,
            links: placed.links,
            bands: placed.bands,
            decorations: Vec::new(),
        });

        page_top = match remaining.as_slice().first() {
            Some(next) if box_top(next).get() < page_bottom => box_top(next).get(),
            _ => page_bottom,
        };

        if settled(&carried, &carried_text, &carried_links)
            && page_top >= content_height
            && remaining.is_empty()
        {
            break;
        }
        if pages.len() >= MAX_PAGES {
            return Err(Error::PageLimitExceeded { limit: MAX_PAGES });
        }
    }

    attach_decorations(
        &tree.decorations,
        &mut pages,
        &page_anchor,
        &piece_map,
        top,
        usable_height,
        warnings,
    )?;

    report_layout_losses(
        tree.borders_not_enclosing,
        tree.borders_on_rows,
        tree.trimmed_leading,
        warnings,
    );

    Ok(Pages {
        pages,
        page_size,
        canvas,
    })
}

fn settled(
    surfaces: &[PlacedRect],
    glyphs: &[PlacedFragment],
    links: &[crate::contracts::PlacedLink],
) -> bool {
    surfaces.is_empty() && glyphs.is_empty() && links.is_empty()
}

fn resume_carried(
    carried: &mut Vec<PlacedRect>,
    bands: &mut Vec<PlacedRect>,
    top: f32,
    usable_height: f32,
) -> Result<(), Error> {
    for rect in std::mem::take(carried) {
        let offset = rect.y.get();

        if offset >= usable_height {
            carried.push(PlacedRect {
                y: Length::try_new(offset - usable_height, "a gap still to cross")?,
                ..rect
            });
            continue;
        }
        let room = (usable_height - offset).max(0.0);
        let height = rect.height.get().min(room);

        if height > 0.0 {
            bands.push(PlacedRect {
                y: Length::try_new(top + offset, "a carried band's top")?,
                height: Length::try_new(height, "a carried band's height")?,
                ..rect
            });
        }

        let left_over = rect.height.get() - height;
        if left_over > 0.0 {
            carried.push(PlacedRect {
                y: Length::try_new(0.0, "a band resuming at a page top")?,
                height: Length::try_new(left_over, "a band still to carry")?,
                ..rect
            });
        }
    }
    Ok(())
}

fn resume_carried_links(
    carried: &mut Vec<crate::contracts::PlacedLink>,
    links: &mut Vec<crate::contracts::PlacedLink>,
    top: f32,
    usable_height: f32,
) -> Result<(), Error> {
    for link in std::mem::take(carried) {
        let offset = link.y.get();
        if offset >= usable_height {
            carried.push(crate::contracts::PlacedLink {
                y: Length::try_new(offset - usable_height, "a link gap still to cross")?,
                ..link
            });
            continue;
        }
        let height = link.height.get().min(usable_height - offset);
        if height > 0.0 {
            links.push(crate::contracts::PlacedLink {
                y: Length::try_new(top + offset, "a carried link's top")?,
                height: Length::try_new(height, "a carried link's height")?,
                ..link.clone()
            });
        }
        let remaining = link.height.get() - height;
        if remaining > 0.0 {
            carried.push(crate::contracts::PlacedLink {
                y: Length::try_new(0.0, "a link resuming at a page top")?,
                height: Length::try_new(remaining, "a link still to carry")?,
                ..link
            });
        }
    }
    Ok(())
}

fn clip_to_page(
    bands: &mut [PlacedRect],
    carried: &mut Vec<PlacedRect>,
    page_floor: f32,
) -> Result<(), Error> {
    for band in bands {
        let bottom = band.y.get() + band.height.get();
        if bottom > page_floor {
            let clipped_at = band.y.get().max(page_floor);
            carried.push(PlacedRect {
                y: Length::try_new(clipped_at - page_floor, "a carried band's offset")?,
                height: Length::try_new(bottom - clipped_at, "a band's offcut")?,
                ..*band
            });
            let visible = (page_floor - band.y.get()).max(0.0);

            band.height = Length::try_new(visible, "a band clipped to its page")?;
        }
    }
    Ok(())
}

fn clip_links_to_page(
    links: &mut [crate::contracts::PlacedLink],
    carried: &mut Vec<crate::contracts::PlacedLink>,
    page_top: f32,
    usable_height: f32,
) -> Result<(), Error> {
    let floor = page_top + usable_height;
    for link in links {
        let bottom = link.y.get() + link.height.get();
        if bottom > floor {
            let clipped_at = link.y.get().max(floor);
            carried.push(crate::contracts::PlacedLink {
                y: Length::try_new(clipped_at - floor, "a carried link's offset")?,
                height: Length::try_new(bottom - clipped_at, "a link's remaining height")?,
                ..link.clone()
            });
        }
        let visible_top = link.y.get().max(page_top);
        let visible_bottom = bottom.min(floor);
        link.y = Length::try_new(visible_top, "a clipped link's top")?;
        link.height = Length::try_new(
            (visible_bottom - visible_top).max(0.0),
            "a clipped link's height",
        )?;
    }
    Ok(())
}

struct PlacedUnit {
    bands: Vec<crate::contracts::PlacedRect>,
    fragments: Vec<PlacedFragment>,
    images: Vec<crate::contracts::PlacedPicture>,
    links: Vec<crate::contracts::PlacedLink>,
}

impl PlacedUnit {
    fn place(unit: FlowUnit, page_top: f32, top: f32) -> Result<Self, Error> {
        let local = |y: f32| {
            Length::try_new(
                y - page_top + top,
                "converting a unit to page-local coordinates",
            )
        };

        match unit.kind() {
            crate::contracts::UnitKind::Line
            | crate::contracts::UnitKind::Row
            | crate::contracts::UnitKind::FlexLine
            | crate::contracts::UnitKind::RowSlice
            | crate::contracts::UnitKind::Image
            | crate::contracts::UnitKind::BoxAnchor => {}
        }

        let mut bands = Vec::new();
        for nested in unit.nested() {
            bands.push(crate::contracts::PlacedRect {
                x: nested.left,
                y: local(nested.top.get())?,
                width: nested.width,
                height: nested.height,
                colour: nested.colour,
            });
        }
        let own = match unit.band() {
            crate::contracts::Band::Canvas => None,
            crate::contracts::Band::Painted {
                colour,
                left,
                width,
            } => Some(crate::contracts::PlacedRect {
                x: left,
                y: local(unit.top().get())?,
                width,
                height: unit.height(),
                colour,
            }),
        };

        if let Some(own) = own {
            bands.insert(0, own);
        }

        let (unit_fragments, unit_images, unit_links) = unit.into_contents();
        let mut images = Vec::new();
        for image in unit_images {
            images.push(crate::contracts::PlacedPicture {
                asset: image.asset,
                x: image.x,
                y: local(image.y.get())?,
                width: image.width,
                height: image.height,
            });
        }
        let mut fragments = Vec::new();
        for fragment in unit_fragments {
            fragments.push(PlacedFragment {
                text: fragment.text,
                x: fragment.x,
                y: local(fragment.y.get())?,
                line_height: fragment.line_height,
                font_size: fragment.font_size,
                face: fragment.face,
                positioned: fragment.positioned,
                colour: fragment.colour,

                clip: None,
                continued: false,
            });
        }
        let mut links = Vec::new();
        for link in unit_links {
            links.push(crate::contracts::PlacedLink {
                target: link.target,
                x: link.x,
                y: local(link.y.get())?,
                width: link.width,
                height: link.height,
            });
        }
        Ok(Self {
            bands,
            fragments,
            images,
            links,
        })
    }

    fn into_parts(
        self,
    ) -> (
        Vec<crate::contracts::PlacedRect>,
        Vec<PlacedFragment>,
        Vec<crate::contracts::PlacedPicture>,
        Vec<crate::contracts::PlacedLink>,
    ) {
        (self.bands, self.fragments, self.images, self.links)
    }
}
