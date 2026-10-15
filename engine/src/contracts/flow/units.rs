use crate::contracts::{Error, FontSize, Length, LinkTarget, Rgb};

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct FlowFragment {
    pub text: String,
    pub x: Length,

    pub y: Length,

    pub line_height: Length,
    pub font_size: FontSize,
    pub face: crate::metrics::Face,
    pub(crate) positioned: Option<crate::font::PositionedRun>,

    pub colour: Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowImage {
    pub asset: usize,
    pub x: Length,

    pub y: Length,
    pub width: Length,
    pub height: Length,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FlowLink {
    pub(crate) target: LinkTarget,
    pub(crate) x: Length,
    pub(crate) y: Length,
    pub(crate) width: Length,
    pub(crate) height: Length,
}

impl FlowLink {
    pub(crate) fn new(
        target: LinkTarget,
        x: Length,
        y: Length,
        width: Length,
        height: Length,
    ) -> Option<Self> {
        (width.get() > 0.0 && height.get() > 0.0).then_some(Self {
            target,
            x,
            y,
            width,
            height,
        })
    }

    pub(crate) fn merge_adjacent(&mut self, next: &Self) -> Result<bool, Error> {
        let right = self.x.get() + self.width.get();
        if self.target != next.target || (right - next.x.get()).abs() > f32::EPSILON {
            return Ok(false);
        }
        let top = self.y.get().min(next.y.get());
        let bottom = (self.y.get() + self.height.get()).max(next.y.get() + next.height.get());
        self.y = Length::try_new(top, "coalescing a link rectangle's top")?;
        self.width = Length::try_new(
            next.x.get() + next.width.get() - self.x.get(),
            "coalescing a link rectangle's width",
        )?;
        self.height = Length::try_new(bottom - top, "coalescing a link rectangle's height")?;
        Ok(true)
    }

    pub(crate) fn rebased(mut self, delta: f32) -> Result<Self, Error> {
        self.y = Length::try_new(self.y.get() + delta, "moving a link down its flow")?;
        Ok(self)
    }
}

#[derive(Debug, PartialEq)]
pub struct FlowUnit {
    kind: UnitKind,
    top: Length,
    height: Length,
    band: Band,

    nested: Vec<FlowBand>,
    fragments: Vec<FlowFragment>,

    images: Vec<FlowImage>,

    links: Vec<FlowLink>,
}

struct UnitContents {
    fragments: Vec<FlowFragment>,
    images: Vec<FlowImage>,
    links: Vec<FlowLink>,
}

impl Band {
    pub fn of(surface: crate::contracts::Surface, left: f32, width: f32) -> Result<Self, Error> {
        match surface {
            crate::contracts::Surface::Canvas => Ok(Self::Canvas),
            crate::contracts::Surface::Colour(colour) => Ok(Self::Painted {
                colour,
                left: Length::try_new(left, "a band's left edge")?,
                width: Length::try_new(width.max(0.0), "a band's width")?,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Band {
    Canvas,
    Painted {
        colour: Rgb,

        left: Length,
        width: Length,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Line,

    Image,
    Row,

    FlexLine,

    RowSlice,

    BoxAnchor,
}

#[derive(Debug, PartialEq)]
pub(crate) struct FlexItemFlow {
    units: Vec<FlowUnit>,
    offset: f32,
}

impl FlexItemFlow {
    pub(crate) fn new(units: Vec<FlowUnit>, offset: f32) -> Self {
        Self { units, offset }
    }
}

#[derive(Debug, PartialEq)]
pub struct CellFlow {
    units: Vec<FlowUnit>,
}

impl CellFlow {
    #[must_use]
    pub const fn new(units: Vec<FlowUnit>) -> Self {
        Self { units }
    }
}

#[derive(Debug)]
pub(crate) enum RowOutcome {
    Whole(FlowUnit),
    Sliced(RowSlices),
}

#[derive(Debug)]
pub(crate) struct RowSlices {
    slices: Vec<FlowUnit>,

    placement: Vec<Vec<Placed>>,
    extent: Length,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Placed {
    slice: usize,
    delta: f32,
}

impl RowSlices {
    #[must_use]
    pub(crate) const fn extent(&self) -> Length {
        self.extent
    }
}

#[derive(Debug)]
pub(crate) struct RowPlacement {
    placement: Vec<Vec<Placed>>,
    first: usize,
}

impl RowPlacement {
    #[must_use]
    pub(crate) fn slices_of(&self, cell: usize, units: &[usize]) -> Vec<usize> {
        let mut found: Vec<usize> = units
            .iter()
            .filter_map(|unit| self.placement.get(cell).and_then(|row| row.get(*unit)))
            .map(|placed| self.first.saturating_add(placed.slice))
            .collect();
        found.sort_unstable();
        found.dedup();
        found
    }

    #[must_use]
    pub(crate) fn delta_of(&self, cell: usize, units: &[usize]) -> Option<f32> {
        units
            .first()
            .and_then(|unit| self.placement.get(cell).and_then(|row| row.get(*unit)))
            .map(|placed| placed.delta)
    }
}

type SliceParts = (
    Vec<FlowFragment>,
    Vec<FlowImage>,
    Vec<FlowLink>,
    Vec<FlowBand>,
    f32,
);

fn build_slice(
    columns: &[Vec<FlowUnit>],
    taken: &[usize],
    consumed: &[usize],
    slice_top: f32,
    placement: &mut [Vec<Placed>],
    ordinal: usize,
) -> Result<SliceParts, Error> {
    let mut fragments = Vec::new();
    let mut images = Vec::new();
    let mut links = Vec::new();
    let mut nested = Vec::new();
    let mut height = 0.0_f32;
    for (column_index, column) in columns.iter().enumerate() {
        let done = taken.get(column_index).copied().unwrap_or(0);
        let count = consumed.get(column_index).copied().unwrap_or(0);
        let Some(prefix) = column.get(done..done.saturating_add(count)) else {
            continue;
        };
        let Some(first) = prefix.first() else {
            continue;
        };

        let delta = slice_top - first.top().get();
        for (offset, unit) in prefix.iter().enumerate() {
            let placed = unit.duplicate().rebased(delta)?;
            height = height.max(placed.top().get() + placed.height().get() - slice_top);
            nested.extend(placed.all_bands());
            let (unit_fragments, unit_images, unit_links) = placed.into_contents();
            fragments.extend(unit_fragments);
            images.extend(unit_images);
            links.extend(unit_links);
            if let Some(row) = placement.get_mut(column_index)
                && let Some(slot) = row.get_mut(done.saturating_add(offset))
            {
                *slot = Placed {
                    slice: ordinal,
                    delta,
                };
            }
        }
    }
    Ok((fragments, images, links, nested, height))
}

fn prefix_that_fits(column: &[FlowUnit], from: usize, usable: f32) -> usize {
    let Some(rest) = column.get(from..) else {
        return 0;
    };
    let Some(first) = rest.first() else { return 0 };
    let start = first.top().get();
    let mut count = 0_usize;
    for unit in rest {
        let extent = unit.top().get() + unit.height().get() - start;
        if !crate::contracts::UsableHeight::fits(extent, usable) {
            break;
        }
        count = count.saturating_add(1);
    }
    count
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowBand {
    pub top: Length,
    pub height: Length,
    pub left: Length,
    pub width: Length,
    pub colour: Rgb,
}

impl FlowUnit {
    pub fn line(
        top: Length,
        height: Length,
        band: Band,
        fragments: Vec<FlowFragment>,
    ) -> Result<Self, Error> {
        Self::new(
            UnitKind::Line,
            top,
            height,
            band,
            Vec::new(),
            UnitContents {
                fragments,
                images: Vec::new(),
                links: Vec::new(),
            },
        )
    }

    pub(crate) fn line_with_links(
        top: Length,
        height: Length,
        band: Band,
        fragments: Vec<FlowFragment>,
        links: Vec<FlowLink>,
    ) -> Result<Self, Error> {
        Self::new(
            UnitKind::Line,
            top,
            height,
            band,
            Vec::new(),
            UnitContents {
                fragments,
                images: Vec::new(),
                links,
            },
        )
    }

    pub fn box_anchor(top: Length, height: Length, band: Band) -> Result<Self, Error> {
        Self::new(
            UnitKind::BoxAnchor,
            top,
            height,
            band,
            Vec::new(),
            UnitContents {
                fragments: Vec::new(),
                images: Vec::new(),
                links: Vec::new(),
            },
        )
    }

    pub fn row(
        top: Length,
        height: Length,
        band: Band,
        cells: Vec<CellFlow>,
    ) -> Result<Self, Error> {
        let mut nested = Vec::new();
        let mut fragments = Vec::new();

        let mut images = Vec::new();
        let mut links = Vec::new();
        for cell in cells {
            for unit in cell.units {
                let placed = unit.rebased(top.get())?;
                nested.extend(placed.all_bands());
                let (cell_fragments, cell_images, cell_links) = placed.into_contents();
                fragments.extend(cell_fragments);
                images.extend(cell_images);
                links.extend(cell_links);
            }
        }
        Self::new(
            UnitKind::Row,
            top,
            height,
            band,
            nested,
            UnitContents {
                fragments,
                images,
                links,
            },
        )
    }

    pub(crate) fn flex_line(
        top: Length,
        height: Length,
        band: Band,
        items: Vec<FlexItemFlow>,
    ) -> Result<Self, Error> {
        let mut nested = Vec::new();
        let mut fragments = Vec::new();
        let mut images = Vec::new();
        let mut links = Vec::new();
        for item in items {
            for unit in item.units {
                let placed = unit.rebased(top.get() + item.offset)?;
                nested.extend(placed.all_bands());
                let (item_fragments, item_images, item_links) = placed.into_contents();
                fragments.extend(item_fragments);
                images.extend(item_images);
                links.extend(item_links);
            }
        }
        Self::new(
            UnitKind::FlexLine,
            top,
            height,
            band,
            nested,
            UnitContents {
                fragments,
                images,
                links,
            },
        )
    }

    pub(crate) fn inline_line(
        top: Length,
        height: Length,
        band: Band,
        items: Vec<FlexItemFlow>,
    ) -> Result<Self, Error> {
        let mut nested = Vec::new();
        let mut fragments = Vec::new();
        let mut images = Vec::new();
        let mut links = Vec::new();
        for item in items {
            for unit in item.units {
                let placed = unit.rebased(top.get() + item.offset)?;
                nested.extend(placed.all_bands());
                let (item_fragments, item_images, item_links) = placed.into_contents();
                fragments.extend(item_fragments);
                images.extend(item_images);
                links.extend(item_links);
            }
        }
        Self::new(
            UnitKind::Line,
            top,
            height,
            band,
            nested,
            UnitContents {
                fragments,
                images,
                links,
            },
        )
    }

    pub(crate) fn row_slices(
        top: Length,
        height: Length,
        band: Band,
        cells: Vec<CellFlow>,
        usable: crate::contracts::UsableHeight,
    ) -> Result<RowOutcome, Error> {
        let usable = usable.get();
        if crate::contracts::UsableHeight::fits(height.get(), usable) {
            return Ok(RowOutcome::Whole(Self::row(top, height, band, cells)?));
        }

        let columns: Vec<Vec<Self>> = cells.into_iter().map(|cell| cell.units).collect();
        let mut taken: Vec<usize> = vec![0; columns.len()];
        let mut placement: Vec<Vec<Placed>> = columns
            .iter()
            .map(|c| vec![Placed::default(); c.len()])
            .collect();
        let mut slices: Vec<Self> = Vec::new();
        let mut slice_top = top.get();

        while taken
            .iter()
            .zip(&columns)
            .any(|(done, all)| *done < all.len())
        {
            let mut consumed: Vec<usize> = Vec::with_capacity(columns.len());
            for (column, done) in columns.iter().zip(&taken) {
                let fits = prefix_that_fits(column, *done, usable);

                let remaining = column.len().saturating_sub(*done);
                consumed.push(if fits == 0 && remaining > 0 { 1 } else { fits });
            }

            if consumed.iter().all(|count| *count == 0) {
                let Some(index) = taken
                    .iter()
                    .zip(&columns)
                    .position(|(done, all)| *done < all.len())
                else {
                    break;
                };
                if let Some(slot) = consumed.get_mut(index) {
                    *slot = 1;
                }
            }

            let built = build_slice(
                &columns,
                &taken,
                &consumed,
                slice_top,
                &mut placement,
                slices.len(),
            )?;
            let (fragments, images, links, nested, slice_height) = built;
            for (column_index, count) in consumed.iter().enumerate() {
                if let Some(slot) = taken.get_mut(column_index) {
                    *slot = slot.saturating_add(*count);
                }
            }
            slices.push(Self::new(
                UnitKind::RowSlice,
                Length::try_new(slice_top, "the top of a row slice")?,
                Length::try_new(slice_height, "the height of a row slice")?,
                band,
                nested,
                UnitContents {
                    fragments,
                    images,
                    links,
                },
            )?);
            slice_top += slice_height;
        }

        let content: f32 = columns
            .iter()
            .map(|column| {
                column
                    .last()
                    .map_or(0.0, |unit| unit.top().get() + unit.height().get())
            })
            .fold(0.0_f32, f32::max);

        if slices.is_empty() {
            return Ok(RowOutcome::Whole(Self::row(top, height, band, Vec::new())?));
        }

        let trailing = (height.get() - content).max(0.0);
        if trailing > 0.0 {
            let last_height = slices.last().map_or(0.0, |slice| slice.height.get());
            let room = (usable - last_height).max(0.0);
            let absorbed = trailing.min(room);
            if absorbed > 0.0
                && let Some(last) = slices.last_mut()
            {
                last.height =
                    Length::try_new(last.height.get() + absorbed, "a row slice's trailing space")?;
            }
            slice_top += trailing;
        }

        Ok(RowOutcome::Sliced(RowSlices {
            slices,
            placement,
            extent: Length::try_new(slice_top - top.get(), "a sliced row's extent")?,
        }))
    }

    #[must_use]
    pub const fn band(&self) -> Band {
        self.band
    }

    pub(crate) fn rebased(self, delta: f32) -> Result<Self, Error> {
        let Self {
            kind,
            top,
            height,
            band,
            nested,
            fragments,
            images,
            links,
        } = self;
        Ok(Self {
            kind,
            top: Length::try_new(top.get() + delta, "moving a unit down its flow")?,
            height,
            band,
            nested: nested
                .into_iter()
                .map(|inherited| {
                    Ok(FlowBand {
                        top: Length::try_new(
                            inherited.top.get() + delta,
                            "moving an inherited band down its flow",
                        )?,
                        ..inherited
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?,
            fragments: fragments
                .into_iter()
                .map(|fragment| {
                    Ok(FlowFragment {
                        y: Length::try_new(
                            fragment.y.get() + delta,
                            "moving a fragment down its flow",
                        )?,
                        ..fragment
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?,

            images: images
                .into_iter()
                .map(|image| {
                    Ok(FlowImage {
                        y: Length::try_new(image.y.get() + delta, "moving an image down its flow")?,
                        ..image
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?,
            links: links
                .into_iter()
                .map(|link| link.rebased(delta))
                .collect::<Result<Vec<_>, Error>>()?,
        })
    }

    fn duplicate(&self) -> Self {
        Self {
            kind: self.kind,
            top: self.top,
            height: self.height,
            band: self.band,
            images: self.images.clone(),
            links: self.links.clone(),
            nested: self.nested.clone(),
            fragments: self
                .fragments
                .iter()
                .map(|fragment| FlowFragment {
                    text: fragment.text.clone(),
                    positioned: fragment.positioned.clone(),
                    ..*fragment
                })
                .collect(),
        }
    }

    fn all_bands(&self) -> Vec<FlowBand> {
        self.as_flow_band()
            .into_iter()
            .chain(self.nested.iter().copied())
            .collect()
    }

    #[must_use]
    pub fn nested(&self) -> &[FlowBand] {
        &self.nested
    }

    #[must_use]
    pub fn as_flow_band(&self) -> Option<FlowBand> {
        match self.band {
            Band::Canvas => None,
            Band::Painted {
                colour,
                left,
                width,
            } => Some(FlowBand {
                top: self.top,
                height: self.height,
                left,
                width,
                colour,
            }),
        }
    }

    fn new(
        kind: UnitKind,
        top: Length,
        height: Length,
        band: Band,
        nested: Vec<FlowBand>,
        contents: UnitContents,
    ) -> Result<Self, Error> {
        if height.get() < 0.0 {
            return Err(Error::NegativeFlowExtent {
                height: height.get(),
            });
        }
        Ok(Self {
            kind,
            top,
            height,
            band,
            nested,
            fragments: contents.fragments,
            images: contents.images,
            links: contents.links,
        })
    }

    pub fn image(top: Length, height: Length, band: Band, image: FlowImage) -> Result<Self, Error> {
        Self::new(
            UnitKind::Image,
            top,
            height,
            band,
            Vec::new(),
            UnitContents {
                fragments: Vec::new(),
                images: vec![image],
                links: Vec::new(),
            },
        )
    }

    pub(crate) fn image_with_link(
        top: Length,
        height: Length,
        band: Band,
        image: FlowImage,
        link: Option<FlowLink>,
    ) -> Result<Self, Error> {
        Self::new(
            UnitKind::Image,
            top,
            height,
            band,
            Vec::new(),
            UnitContents {
                fragments: Vec::new(),
                images: vec![image],
                links: link.into_iter().collect(),
            },
        )
    }

    #[must_use]
    pub const fn kind(&self) -> UnitKind {
        self.kind
    }

    #[must_use]
    pub const fn top(&self) -> Length {
        self.top
    }

    #[must_use]
    pub const fn height(&self) -> Length {
        self.height
    }

    #[must_use]
    pub fn fragments(&self) -> &[FlowFragment] {
        &self.fragments
    }

    #[must_use]
    pub(crate) fn into_contents(self) -> (Vec<FlowFragment>, Vec<FlowImage>, Vec<FlowLink>) {
        (self.fragments, self.images, self.links)
    }
}

const FLOW_ORDER_TOLERANCE: f32 = 0.01;

#[derive(Debug, Default, PartialEq)]
pub struct FlowUnits {
    units: Vec<FlowUnit>,
}

impl FlowUnits {
    #[must_use]
    pub const fn new() -> Self {
        Self { units: Vec::new() }
    }

    pub(crate) fn take_units(&mut self) -> Vec<FlowUnit> {
        std::mem::take(&mut self.units)
    }

    pub(crate) fn push_run(
        &mut self,
        run: RowSlices,
    ) -> Result<(std::ops::Range<usize>, RowPlacement), Error> {
        let first = self.units.len();
        for slice in run.slices {
            self.push(slice)?;
        }
        Ok((
            first..self.units.len(),
            RowPlacement {
                placement: run.placement,
                first,
            },
        ))
    }

    pub fn push(&mut self, unit: FlowUnit) -> Result<(), Error> {
        if let Some(last) = self.units.last() {
            let previous = last.top.get() + last.height.get();
            let next = unit.top.get() + unit.height.get();
            if next < previous - FLOW_ORDER_TOLERANCE {
                return Err(Error::FlowOutOfOrder { previous, next });
            }
        }
        self.units.push(unit);
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.units.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    #[must_use]
    pub fn as_slice(&self) -> &[FlowUnit] {
        &self.units
    }

    pub fn fragments(&self) -> impl Iterator<Item = &FlowFragment> {
        self.units.iter().flat_map(FlowUnit::fragments)
    }

    #[must_use]
    pub fn into_units(self) -> Vec<FlowUnit> {
        self.units
    }

    #[must_use]
    pub fn take_first(&mut self, at: usize) -> Vec<FlowUnit> {
        let rest = self.units.split_off(at.min(self.units.len()));
        std::mem::replace(&mut self.units, rest)
    }
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct FragmentTree {
    pub units: FlowUnits,

    pub(crate) decorations: Vec<crate::layout::Decoration>,

    pub(crate) borders_on_rows: usize,

    pub(crate) borders_not_enclosing: usize,

    pub(crate) trimmed_leading: (usize, f32),

    pub(crate) escaped: crate::layout::Escapes,

    pub(crate) overrun: (usize, f32),

    pub(crate) refitted: (usize, f32),

    pub canvas: Option<crate::contracts::Rgb>,
    pub content_height: Length,
}
