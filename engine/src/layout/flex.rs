use super::blocks::{Collapse, place};
use super::contracts::{Decoration, OwnBorder, Sink};
use super::tables::Intrinsic;
use crate::contracts::{
    BoxFlexBasis, BoxFlexContainer, BoxNode, Error, FlexAlign, FlexItemFlow, FlexJustify, FlowUnit,
    FlowUnits, Length, Spacing, Width,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sizing {
    pub(super) base: f32,
    pub(super) minimum: f32,
    pub(super) grow: f32,
    pub(super) shrink: f32,
}

fn count_as_f32(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

pub(super) fn solve(items: &[Sizing], available: f32, gap: f32) -> Vec<f32> {
    let gaps = gap.max(0.0) * count_as_f32(items.len().saturating_sub(1));
    let budget = (available - gaps).max(0.0);
    let mut sizes: Vec<f32> = items
        .iter()
        .map(|item| item.base.max(item.minimum).max(0.0))
        .collect();
    let total: f32 = sizes.iter().sum();

    if total < budget {
        let factors: f32 = items.iter().map(|item| item.grow).sum();
        if factors > 0.0 {
            let free = budget - total;
            for (size, item) in sizes.iter_mut().zip(items) {
                *size += free * item.grow / factors;
            }
        }
        return sizes;
    }

    let mut deficit = total - budget;
    let mut active: Vec<bool> = items
        .iter()
        .zip(&sizes)
        .map(|(item, size)| item.shrink > 0.0 && *size > item.minimum)
        .collect();
    for _ in 0..items.len() {
        if deficit <= 0.001 {
            break;
        }
        let scaled: f32 = items
            .iter()
            .zip(&sizes)
            .zip(&active)
            .filter(|(_, active)| **active)
            .map(|((item, size), _)| item.shrink * *size)
            .sum();
        if scaled <= 0.0 {
            break;
        }
        let before = deficit;
        for ((item, size), active) in items.iter().zip(&mut sizes).zip(&mut active) {
            if !*active {
                continue;
            }
            let share = before * item.shrink * *size / scaled;
            let reduction = share.min((*size - item.minimum).max(0.0));
            *size -= reduction;
            deficit -= reduction;
            if *size <= item.minimum + 0.001 {
                *size = item.minimum;
                *active = false;
            }
        }
    }
    sizes
}

pub(super) fn positions(sizes: &[f32], available: f32, gap: f32, justify: FlexJustify) -> Vec<f32> {
    let occupied: f32 =
        sizes.iter().sum::<f32>() + gap * count_as_f32(sizes.len().saturating_sub(1));
    let free = (available - occupied).max(0.0);
    let count = sizes.len();
    #[allow(
        clippy::match_same_arms,
        reason = "start and the degenerate single-item distribution modes reach the same numbers for different semantic reasons"
    )]
    let (start, extra) = match justify {
        FlexJustify::Start => (0.0, 0.0),
        FlexJustify::End => (free, 0.0),
        FlexJustify::Center => (free / 2.0, 0.0),
        FlexJustify::SpaceBetween if count > 1 => (0.0, free / count_as_f32(count - 1)),
        FlexJustify::SpaceAround if count > 0 => {
            let share = free / count_as_f32(count);
            (share / 2.0, share)
        }
        FlexJustify::SpaceEvenly if count > 0 => {
            let share = free / count_as_f32(count + 1);
            (share, share)
        }
        FlexJustify::SpaceBetween | FlexJustify::SpaceAround | FlexJustify::SpaceEvenly => {
            (0.0, 0.0)
        }
    };
    let mut cursor = start;
    sizes
        .iter()
        .map(|size| {
            let position = cursor;
            cursor += *size + gap + extra;
            position
        })
        .collect()
}

pub(super) fn inline_width(node: &BoxNode, available: f32) -> Result<f32, Error> {
    let outer = node.margin.left.get()
        + node.margin.right.get()
        + node.padding.left.get()
        + node.padding.right.get()
        + node.border.horizontal();
    let declared = match node.width {
        Width::Auto => None,
        Width::Fixed(value) => Some(value.resolve(node.font_size)?.get()),
        Width::Percent(fraction) => Some(available * fraction),
    };
    let content = if let Some(declared) = declared {
        declared
    } else {
        match node.flex_container.direction {
            crate::contracts::FlexDirection::Row => {
                let children = node.children.iter().filter(|child| {
                !matches!(&child.content, crate::contracts::InlineContent::Text(text) if text.trim().is_empty())
                    || !child.children.is_empty()
            });
                let mut count = 0_usize;
                let bases = children
                    .map(|child| -> Result<f32, Error> {
                        count = count.saturating_add(1);
                        Ok(sizing_of(child, available)?.base)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .sum::<f32>();
                bases + node.flex_container.column_gap.get() * count_as_f32(count.saturating_sub(1))
            }
            crate::contracts::FlexDirection::Column => node
                .children
                .iter()
                .map(|child| cross_width(child, available, FlexAlign::Start))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .fold(0.0_f32, f32::max),
        }
    };
    let width = match (declared, node.box_sizing) {
        (Some(_), crate::contracts::BoxSizing::BorderBox) => {
            content + node.margin.left.get() + node.margin.right.get()
        }
        _ => content + outer,
    };
    Ok(width.min(available).max(0.0))
}

struct LaidItem {
    units: Vec<FlowUnit>,
    decorations: Vec<Decoration>,
    height: f32,
    align: FlexAlign,
    stretch: Option<StretchBox>,
}

struct StretchBox {
    band: crate::contracts::Band,
    border: crate::contracts::BorderEdges,
    left: f32,
    width: f32,
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "row placement needs the container, containing block, flow cursor and output lanes; sizing, local layout and lossless lane consumption stay one paired operation"
)]
pub(super) fn place_row(
    mut children: Vec<BoxNode>,
    container: BoxFlexContainer,
    band: crate::contracts::Band,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut Sink<'_>,
) -> Result<(), Error> {
    children.retain(|child| {
        !matches!(&child.content, crate::contracts::InlineContent::Text(text) if text.trim().is_empty())
            || !child.children.is_empty()
    });
    let gap = container.column_gap.get();
    let sizing: Vec<Sizing> = children
        .iter()
        .map(|child| sizing_of(child, available))
        .collect::<Result<_, _>>()?;
    let sizes = solve(&sizing, available, gap);
    let starts = positions(&sizes, available, gap, container.justify);

    pending.flush_into(cursor);
    let top = *cursor;
    let mut laid = Vec::with_capacity(children.len());
    let mut line_height = 0.0_f32;
    for ((mut child, width), start) in children.into_iter().zip(&sizes).zip(&starts) {
        let horizontal = child.margin.left.get()
            + child.margin.right.get()
            + child.padding.left.get()
            + child.padding.right.get()
            + child.border.horizontal();
        child.width = Width::Fixed(Spacing::points(match child.box_sizing {
            crate::contracts::BoxSizing::ContentBox => (*width - horizontal).max(0.0),
            crate::contracts::BoxSizing::BorderBox => {
                (*width - child.margin.left.get() - child.margin.right.get()).max(0.0)
            }
        }));
        let align = child.flex_item.align_self.unwrap_or(container.align_items);
        let stretch = (align == FlexAlign::Stretch && (child.paints || child.border.any()))
            .then(|| -> Result<StretchBox, Error> {
                let border_left = left + *start + child.margin.left.get();
                let border_width =
                    (*width - child.margin.left.get() - child.margin.right.get()).max(0.0);
                Ok(StretchBox {
                    band: crate::contracts::Band::of(child.surface, border_left, border_width)?,
                    border: child.border,
                    left: border_left,
                    width: border_width,
                })
            })
            .transpose()?;
        if stretch.is_some() {
            child.surface = crate::contracts::Surface::Canvas;
        }
        let mut units = FlowUnits::new();
        let mut local_cursor = 0.0;
        let mut local_pending = Collapse::default();
        let mut decorations = Vec::new();
        let mut local_sink = Sink {
            units: &mut units,
            usable: sink.usable,
            sheet: sink.sheet,
            escaped: sink.escaped,
            overrun: sink.overrun,
            refitted: sink.refitted,
            decorations: &mut decorations,
            ignored_borders: sink.ignored_borders,
            not_enclosing: sink.not_enclosing,
            trimmed_leading: sink.trimmed_leading,
        };
        place(
            child,
            crate::contracts::UsableWidth::span(left + *start, *width)?,
            &mut local_cursor,
            &mut local_pending,
            &mut local_sink,
            if stretch.is_some() {
                OwnBorder::CallerRecords
            } else {
                OwnBorder::Emit
            },
        )?;
        local_pending.flush_into(&mut local_cursor);
        line_height = line_height.max(local_cursor);
        laid.push(LaidItem {
            units: units.take_units(),
            decorations,
            height: local_cursor,
            align,
            stretch,
        });
    }

    for item in &mut laid {
        if let Some(stretch) = &item.stretch
            && !matches!(stretch.band, crate::contracts::Band::Canvas)
        {
            item.units.insert(
                0,
                FlowUnit::box_anchor(
                    Length::try_new(0.0, "the top of a stretched flex item")?,
                    Length::try_new(line_height, "the height of a stretched flex item")?,
                    stretch.band,
                )?,
            );
        }
    }
    let offsets: Vec<f32> = laid
        .iter()
        .map(|item| cross_offset(item.align, line_height, item.height))
        .collect();
    let flows = laid
        .iter_mut()
        .zip(&offsets)
        .map(|(item, offset)| FlexItemFlow::new(std::mem::take(&mut item.units), *offset))
        .collect();
    sink.units.push(FlowUnit::flex_line(
        Length::try_new(top, "the top of a flex line")?,
        Length::try_new(line_height, "the height of a flex line")?,
        band,
        flows,
    )?)?;
    let member = [sink.units.len().saturating_sub(1)];
    for (item, offset) in laid.into_iter().zip(offsets) {
        if let Some(stretch) = item.stretch
            && stretch.border.any()
        {
            match Decoration::new(
                Some(Length::try_new(top, "a stretched flex item's top")?),
                Length::try_new(top + line_height, "a stretched flex item's bottom")?,
                Length::try_new(stretch.left, "a stretched flex item's left")?,
                Length::try_new(stretch.width, "a stretched flex item's width")?,
                stretch.border,
                member.to_vec(),
                sink.units,
            ) {
                Ok(decoration) => sink.decorations.push(decoration),
                Err(Error::DecorationExtentDisagreesWithMembers { .. }) => {
                    *sink.not_enclosing = sink.not_enclosing.saturating_add(1);
                }
                Err(other) => return Err(other),
            }
        }
        for decoration in item.decorations {
            match decoration.rebased(top + offset, &member, sink.units) {
                Ok(rebased) => sink.decorations.push(rebased),
                Err(Error::DecorationExtentDisagreesWithMembers { .. }) => {
                    *sink.not_enclosing = sink.not_enclosing.saturating_add(1);
                }
                Err(other) => return Err(other),
            }
        }
    }
    *cursor += line_height;
    Ok(())
}

pub(super) fn place_column(
    mut children: Vec<BoxNode>,
    container: BoxFlexContainer,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut Sink<'_>,
) -> Result<(), Error> {
    children.retain(|child| {
        !matches!(&child.content, crate::contracts::InlineContent::Text(text) if text.trim().is_empty())
            || !child.children.is_empty()
    });
    pending.flush_into(cursor);
    let count = children.len();
    for (index, mut child) in children.into_iter().enumerate() {
        let align = child.flex_item.align_self.unwrap_or(container.align_items);
        let outer_width = cross_width(&child, available, align)?;
        let start = match align {
            FlexAlign::Stretch | FlexAlign::Start => 0.0,
            FlexAlign::End => available - outer_width,
            FlexAlign::Center => (available - outer_width) / 2.0,
        }
        .max(0.0);
        let horizontal = child.margin.left.get()
            + child.margin.right.get()
            + child.padding.left.get()
            + child.padding.right.get()
            + child.border.horizontal();
        child.width = Width::Fixed(Spacing::points(match child.box_sizing {
            crate::contracts::BoxSizing::ContentBox => (outer_width - horizontal).max(0.0),
            crate::contracts::BoxSizing::BorderBox => {
                (outer_width - child.margin.left.get() - child.margin.right.get()).max(0.0)
            }
        }));
        let mut fresh = Collapse::default();
        place(
            child,
            crate::contracts::UsableWidth::span(left + start, outer_width)?,
            cursor,
            &mut fresh,
            sink,
            OwnBorder::Emit,
        )?;
        fresh.flush_into(cursor);
        if index + 1 < count {
            *cursor += container.row_gap.get();
        }
    }
    Ok(())
}

fn sizing_of(child: &BoxNode, available: f32) -> Result<Sizing, Error> {
    let intrinsic = Intrinsic::content_of(child);
    let horizontal = child.margin.left.get() + child.margin.right.get();
    let inset = child.padding.left.get() + child.padding.right.get() + child.border.horizontal();
    let declared = match child.width {
        Width::Auto => None,
        Width::Fixed(value) => Some(value.resolve(child.font_size)?.get()),
        Width::Percent(fraction) => Some(available * fraction),
    };
    let declared_content = declared.map(|value| match child.box_sizing {
        crate::contracts::BoxSizing::ContentBox => value,
        crate::contracts::BoxSizing::BorderBox => (value - inset).max(0.0),
    });
    let basis_content = |value: f32| match child.box_sizing {
        crate::contracts::BoxSizing::ContentBox => value,
        crate::contracts::BoxSizing::BorderBox => (value - inset).max(0.0),
    };
    let content_basis = match child.flex_item.basis {
        BoxFlexBasis::Auto => declared_content.unwrap_or((intrinsic.max - inset).max(0.0)),
        BoxFlexBasis::Content => (intrinsic.max - inset).max(0.0),
        BoxFlexBasis::Fixed(value) => basis_content(value.get()),
        BoxFlexBasis::Percent(fraction) => basis_content(available * fraction),
    };
    Ok(Sizing {
        base: content_basis + inset + horizontal,
        minimum: intrinsic.min + horizontal,
        grow: child.flex_item.grow.get(),
        shrink: child.flex_item.shrink.get(),
    })
}

fn cross_width(child: &BoxNode, available: f32, align: FlexAlign) -> Result<f32, Error> {
    let horizontal = child.margin.left.get() + child.margin.right.get();
    let inset = child.padding.left.get() + child.padding.right.get() + child.border.horizontal();
    let declared = match child.width {
        Width::Auto => None,
        Width::Fixed(value) => Some(value.resolve(child.font_size)?.get()),
        Width::Percent(fraction) => Some(available * fraction),
    };
    Ok(if align == FlexAlign::Stretch && declared.is_none() {
        available
    } else {
        declared
            .map_or_else(
                || Intrinsic::content_of(child).max + horizontal,
                |value| match child.box_sizing {
                    crate::contracts::BoxSizing::ContentBox => value + inset + horizontal,
                    crate::contracts::BoxSizing::BorderBox => value + horizontal,
                },
            )
            .min(available)
    })
}

const fn cross_offset(align: FlexAlign, line: f32, item: f32) -> f32 {
    match align {
        FlexAlign::Stretch | FlexAlign::Start => 0.0,
        FlexAlign::End => line - item,
        FlexAlign::Center => (line - item) / 2.0,
    }
}
