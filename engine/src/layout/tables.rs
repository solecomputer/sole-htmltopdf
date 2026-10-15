use super::blocks::{Collapse, Extent, place, record_border};
use super::contracts::{Decoration, OwnBorder, Sink};
use super::inline::{BoxStyle, Run, tokenize};
use crate::contracts::{BoxNode, CellFlow, Display, Error, FlowUnit, FlowUnits, Length};

fn place_row(
    cells: Vec<BoxNode>,
    surface: crate::contracts::Surface,
    left: f32,
    widths: &[f32],
    cursor: &mut f32,
    sink: &mut Sink<'_>,
) -> Result<(), Error> {
    let top = *cursor;
    let mut flows: Vec<CellFlow> = Vec::new();
    let mut height = 0.0_f32;
    let mut x = left + BORDER_SPACING;

    let mut held: Vec<HeldCell> = Vec::new();
    for (index, cell) in cells.into_iter().enumerate() {
        let column = widths.get(index).copied().unwrap_or(0.0);
        let mut local = FlowUnits::new();
        let mut local_cursor = 0.0_f32;

        let mut fresh = Collapse::default();
        let border = cell.border;
        let mut inner = Vec::new();
        let mut cell_sink = Sink {
            sheet: sink.sheet,
            escaped: sink.escaped,
            overrun: sink.overrun,
            refitted: sink.refitted,
            units: &mut local,

            usable: sink.usable,
            decorations: &mut inner,
            ignored_borders: sink.ignored_borders,
            not_enclosing: sink.not_enclosing,

            trimmed_leading: sink.trimmed_leading,
        };
        place(
            cell,
            crate::contracts::UsableWidth::span(x, column)?,
            &mut local_cursor,
            &mut fresh,
            &mut cell_sink,
            OwnBorder::CallerRecords,
        )?;
        fresh.flush_into(&mut local_cursor);
        flows.push(retain_cell(local));
        held.push(HeldCell {
            left: x,
            width: column,
            border,
            inner,
        });
        height = height.max(local_cursor);
        x += column + BORDER_SPACING;
    }

    let band_width = (x - BORDER_SPACING - (left + BORDER_SPACING)).max(0.0);
    let outcome = FlowUnit::row_slices(
        Length::try_new(top, "the top of a row")?,
        Length::try_new(height, "the height of a row")?,
        crate::contracts::Band::of(surface, left + BORDER_SPACING, band_width)?,
        flows,
        sink.usable,
    )?;

    let (members, emitted, mapping) = match outcome {
        crate::contracts::RowOutcome::Whole(row) => {
            sink.units.push(row)?;
            (vec![sink.units.len().saturating_sub(1)], height, None)
        }
        crate::contracts::RowOutcome::Sliced(run) => {
            let extent = run.extent().get();
            let (range, placement) = sink.units.push_run(run)?;
            (range.collect(), extent, Some(placement))
        }
    };
    attach_cell_decorations(held, &members, mapping.as_ref(), top, emitted, sink)?;

    *cursor += emitted;
    Ok(())
}

struct HeldCell {
    left: f32,
    width: f32,
    border: crate::contracts::BorderEdges,

    inner: Vec<Decoration>,
}

pub(super) fn place_table(
    node: BoxNode,
    outer_left: f32,
    outer_width: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut Sink<'_>,
) -> Result<(), Error> {
    let BoxNode {
        children,
        margin,
        padding,
        border,
        width,
        font_size,
        ..
    } = node;

    let first_unit = sink.units.len();
    pending.absorb(margin.top.get());
    let border_box_top = if border.top.used_width() > 0.0 || padding.top.get() > 0.0 {
        pending.flush_into(cursor);
        let top = *cursor;
        *cursor += border.top.used_width() + padding.top.get();
        Some(top)
    } else {
        None
    };

    pending.flush_into(cursor);

    let left_inset = border.left.used_width() + padding.left.get();
    let right_inset = border.right.used_width() + padding.right.get();
    let left = outer_left + margin.left.get() + left_inset;
    let filled = outer_width - margin.left.get() - margin.right.get() - left_inset - right_inset;
    let declared_content = |declared: f32| match node.box_sizing {
        crate::contracts::BoxSizing::ContentBox => declared,
        crate::contracts::BoxSizing::BorderBox => (declared - left_inset - right_inset).max(0.0),
    };
    let available = match width {
        crate::contracts::Width::Auto => filled,
        crate::contracts::Width::Fixed(spacing) => {
            declared_content(spacing.resolve(font_size)?.get())
        }
        crate::contracts::Width::Percent(fraction) => declared_content(outer_width * fraction),
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut strays: Vec<BoxNode> = Vec::new();
    collect_rows(children, &mut rows, &mut strays, sink.ignored_borders);

    for stray in strays {
        place(
            stray,
            crate::contracts::UsableWidth::span(left, available)?,
            cursor,
            pending,
            sink,
            OwnBorder::Emit,
        )?;
    }

    let mut solved = available;
    if !rows.is_empty() {
        let columns = rows.iter().map(|(_, row)| row.len()).max().unwrap_or(0);
        let mut intrinsics = vec![Intrinsic { min: 0.0, max: 0.0 }; columns];
        for (_, row) in &rows {
            for (index, cell) in row.iter().enumerate() {
                let Some(column) = intrinsics.get_mut(index) else {
                    continue;
                };
                let cell_width = Intrinsic::of(cell);
                column.min = column.min.max(cell_width.min);
                column.max = column.max.max(cell_width.max);
            }
        }
        let used = used_table_width(
            available,
            &intrinsics,
            matches!(width, crate::contracts::Width::Auto),
        );
        solved = used;
        let widths = solve_columns(&intrinsics, used);

        *cursor += BORDER_SPACING;
        for (surface, cells) in rows {
            place_row(cells, surface, left, &widths, cursor, sink)?;
            *cursor += BORDER_SPACING;
        }
    }

    if border.bottom.used_width() > 0.0 || padding.bottom.get() > 0.0 {
        pending.flush_into(cursor);
        *cursor += border.bottom.used_width() + padding.bottom.get();
    }

    record_border(
        border,
        node.surface,
        node.paints,
        sink,
        OwnBorder::Emit,
        Extent {
            first_unit,
            top: border_box_top,

            empty_top: border_box_top.unwrap_or(*cursor),
            bottom: *cursor,
            left: left - left_inset,
            width: solved + left_inset + right_inset,
        },
    )?;

    pending.absorb(margin.bottom.get());
    Ok(())
}

pub(super) type Row = (crate::contracts::Surface, Vec<BoxNode>);

pub(super) fn collect_rows(
    children: Vec<BoxNode>,
    out: &mut Vec<Row>,
    strays: &mut Vec<BoxNode>,
    ignored_borders: &mut usize,
) {
    for child in children {
        if matches!(child.display, Display::Row | Display::RowGroup) && child.border.any() {
            *ignored_borders = ignored_borders.saturating_add(1);
        }
        match child.display {
            Display::Row => {
                let surface = child.surface;
                out.push((surface, cells_of(child)));
            }
            Display::RowGroup => collect_rows(child.children, out, strays, ignored_borders),

            Display::Block
            | Display::Inline
            | Display::Flex
            | Display::InlineFlex
            | Display::Table
            | Display::Cell => {
                strays.push(child);
            }
        }
    }
}

pub(super) fn cells_of(row: BoxNode) -> Vec<BoxNode> {
    let mut cells: Vec<BoxNode> = Vec::new();
    let mut pending_anonymous: Vec<BoxNode> = Vec::new();
    for child in row.children {
        if child.display == Display::Cell {
            flush_anonymous(&mut pending_anonymous, &mut cells);
            cells.push(child);
            continue;
        }
        let blank = child.children.is_empty() && {
            #[allow(
                clippy::match_same_arms,
                reason = "a break and an image are not blank for different reasons; \
                              merging them couples two unrelated rules"
            )]
            match &child.content {
                crate::contracts::InlineContent::Text(text) => text.trim().is_empty(),

                crate::contracts::InlineContent::ForcedBreak => false,

                crate::contracts::InlineContent::Replaced(_)
                | crate::contracts::InlineContent::Placed(_) => false,
                crate::contracts::InlineContent::Empty => true,
            }
        };
        if blank {
            continue;
        }
        pending_anonymous.push(child);
    }
    flush_anonymous(&mut pending_anonymous, &mut cells);
    cells
}

fn flush_anonymous(run: &mut Vec<BoxNode>, cells: &mut Vec<BoxNode>) {
    if run.is_empty() {
        return;
    }
    let children = std::mem::take(run);
    let Some(first) = children.first() else {
        return;
    };
    cells.push(BoxNode {
        box_sizing: crate::contracts::BoxSizing::ContentBox,
        flex_container: crate::contracts::BoxFlexContainer::default(),
        flex_item: crate::contracts::BoxFlexItem::default(),
        link: None,
        content: crate::contracts::InlineContent::Empty,

        border: crate::contracts::BorderEdges::default(),
        font_size: first.font_size,
        margin: crate::contracts::Edges::zero(),
        padding: crate::contracts::Edges::zero(),
        line_height: first.line_height,
        authored_face: first.authored_face.clone(),

        colour: first.colour,
        surface: first.surface,
        paints: first.paints,
        width: crate::contracts::Width::Auto,
        display: Display::Cell,
        text_align: first.text_align,
        face: first.face,
        children,
    });
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Intrinsic {
    pub(super) min: f32,

    pub(super) max: f32,
}

impl Intrinsic {
    pub(super) fn of(node: &BoxNode) -> Self {
        Self::content_of(node).with_declared_width(node)
    }

    pub(super) fn content_of(node: &BoxNode) -> Self {
        let mut widest = Self { min: 0.0, max: 0.0 };
        let mut inline: Vec<Run> = Vec::new();
        Self::walk(node, &mut inline, &mut widest);

        widest.absorb(Self::of_runs(&inline));
        widest.plus_padding(node)
    }

    pub(super) fn walk(node: &BoxNode, inline: &mut Vec<Run>, widest: &mut Self) {
        let style = BoxStyle {
            link: node.link.clone(),
            face: node.face,
            authored_face: node.authored_face.clone(),
            font_size: node.font_size,
            line_height: node.line_height,
            text_align: node.text_align,
            colour: node.colour,
            surface: node.surface,

            band: crate::contracts::Band::Canvas,
        };

        match &node.content {
            crate::contracts::InlineContent::Text(text) => {
                inline.push(Run::of(&style, text.clone()));
            }

            crate::contracts::InlineContent::Placed(image) => {
                widest.absorb(Self {
                    min: image.width,
                    max: image.width,
                });
            }

            #[allow(
                clippy::match_same_arms,
                reason = "an unresolved request and an empty box contribute nothing for \
                          different reasons; merging couples two unrelated rules"
            )]
            crate::contracts::InlineContent::Replaced(_) => {}
            crate::contracts::InlineContent::Empty
            | crate::contracts::InlineContent::ForcedBreak => {}
        }
        for child in &node.children {
            if child.content == crate::contracts::InlineContent::ForcedBreak {
                widest.absorb(Self::of_runs(inline));
                inline.clear();
                continue;
            }
            match child.display {
                Display::Inline => Self::walk(child, inline, widest),
                Display::Block
                | Display::Flex
                | Display::InlineFlex
                | Display::Table
                | Display::RowGroup
                | Display::Row
                | Display::Cell => {
                    widest.absorb(Self::of_runs(inline));
                    inline.clear();
                    let mut nested: Vec<Run> = Vec::new();
                    let mut inner = Self { min: 0.0, max: 0.0 };
                    Self::walk(child, &mut nested, &mut inner);
                    inner.absorb(Self::of_runs(&nested));
                    widest.absorb(inner.with_declared_width(child).plus_padding(child));
                }
            }
        }
    }

    pub(super) fn absorb(&mut self, other: Self) {
        self.min = self.min.max(other.min);
        self.max = self.max.max(other.max);
    }

    pub(super) fn of_runs(runs: &[Run]) -> Self {
        let mut min = 0.0_f32;
        let mut max = 0.0_f32;
        for token in &tokenize(runs) {
            let Some(_run) = runs.get(token.run) else {
                continue;
            };
            let word = token.word_advance;
            min = min.max(word);

            if token.space_before && max > 0.0 {
                max += token.space_advance;
            }
            max += word;
        }
        Self { min, max }
    }

    pub(super) fn plus_padding(self, node: &BoxNode) -> Self {
        let extra = node.padding.left.get() + node.padding.right.get() + node.border.horizontal();
        Self {
            min: self.min + extra,
            max: self.max + extra,
        }
    }

    pub(super) fn with_declared_width(self, node: &BoxNode) -> Self {
        let crate::contracts::Width::Fixed(spacing) = node.width else {
            return self;
        };
        let Ok(declared) = spacing.resolve(node.font_size) else {
            return self;
        };
        let content = match node.box_sizing {
            crate::contracts::BoxSizing::ContentBox => declared.get(),
            crate::contracts::BoxSizing::BorderBox => (declared.get()
                - node.padding.left.get()
                - node.padding.right.get()
                - node.border.horizontal())
            .max(0.0),
        };
        Self {
            min: self.min.max(content),
            max: self.max.max(content),
        }
    }
}

pub(super) const BORDER_SPACING: f32 = 1.5;

pub(super) fn used_table_width(declared: f32, intrinsics: &[Intrinsic], is_auto: bool) -> f32 {
    let gaps = spacing_total(intrinsics.len());
    let total_min: f32 = intrinsics.iter().map(|column| column.min).sum::<f32>() + gaps;
    let total_max: f32 = intrinsics.iter().map(|column| column.max).sum::<f32>() + gaps;
    if is_auto {
        total_max.min(declared).max(total_min)
    } else {
        declared.max(total_min)
    }
}

fn spacing_total(columns: usize) -> f32 {
    u16::try_from(columns.saturating_add(1)).map_or(0.0, |gaps| f32::from(gaps) * BORDER_SPACING)
}

pub(super) fn solve_columns(intrinsics: &[Intrinsic], used: f32) -> Vec<f32> {
    let used = used - spacing_total(intrinsics.len());
    let total_min: f32 = intrinsics.iter().map(|column| column.min).sum();
    let total_want: f32 = intrinsics
        .iter()
        .map(|column| (column.max - column.min).max(0.0))
        .sum();
    let slack = (used - total_min).max(0.0);

    let shared = slack.min(total_want);
    let mut widths: Vec<f32> = intrinsics
        .iter()
        .map(|column| {
            let want = (column.max - column.min).max(0.0);
            if total_want <= 0.0 {
                column.min
            } else {
                column.min + shared * (want / total_want)
            }
        })
        .collect();

    let residue = slack - shared;
    if residue > 0.0 {
        let total_max: f32 = intrinsics.iter().map(|column| column.max.max(0.0)).sum();
        for (width, column) in widths.iter_mut().zip(intrinsics) {
            if total_max > 0.0 {
                *width += residue * (column.max.max(0.0) / total_max);
            } else if let Some(count) = u16::try_from(intrinsics.len())
                .ok()
                .filter(|count| *count > 0)
            {
                *width += residue / f32::from(count);
            }
        }
    }
    widths
}

fn retain_cell(local: FlowUnits) -> CellFlow {
    CellFlow::new(local.into_units())
}

fn attach_cell_decorations(
    held: Vec<HeldCell>,
    members: &[usize],
    mapping: Option<&crate::contracts::RowPlacement>,
    top: f32,
    emitted: f32,
    sink: &mut Sink<'_>,
) -> Result<(), Error> {
    for (index, cell) in held.into_iter().enumerate() {
        for decoration in cell.inner {
            let (named, offset) = match mapping {
                Some(run) => {
                    let slices = run.slices_of(index, decoration.members());
                    let moved_as_one = slices.len() == 1;
                    let offset = if moved_as_one {
                        run.delta_of(index, decoration.members()).unwrap_or(top)
                    } else {
                        top
                    };
                    (slices, offset)
                }
                None => (members.to_vec(), top),
            };

            match decoration.rebased(offset, &named, sink.units) {
                Ok(rebased) => sink.decorations.push(rebased),
                Err(Error::DecorationExtentDisagreesWithMembers { .. }) => {
                    *sink.not_enclosing = sink.not_enclosing.saturating_add(1);
                }
                Err(other) => return Err(other),
            }
        }

        if cell.border.any() {
            match Decoration::new(
                Some(Length::try_new(top, "a cell decoration's top")?),
                Length::try_new(top + emitted, "a cell decoration's bottom")?,
                Length::try_new(cell.left, "a cell decoration's left")?,
                Length::try_new(cell.width, "a cell decoration's width")?,
                cell.border,
                members.to_vec(),
                sink.units,
            ) {
                Ok(decoration) => sink.decorations.push(decoration),
                Err(Error::DecorationExtentDisagreesWithMembers { .. }) => {
                    *sink.not_enclosing = sink.not_enclosing.saturating_add(1);
                }
                Err(other) => return Err(other),
            }
        }
    }

    Ok(())
}
