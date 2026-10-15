use super::contracts::{Decoration, OwnBorder, Sink};
use super::inline::{BoxStyle, InlinePiece, Run, emit_pieces, gather};
use super::tables::place_table;
use crate::contracts::{
    BoxNode, BoxTree, Display, Error, FlowUnit, FlowUnits, FragmentTree, InlineContent, Length,
};

pub(crate) fn flow(
    boxes: BoxTree,
    measure: crate::contracts::UsableWidth,

    sheet: crate::contracts::UsableWidth,
    usable: crate::contracts::UsableHeight,
) -> Result<FragmentTree, Error> {
    let mut units = FlowUnits::new();

    let mut cursor = 0.0_f32;

    let mut pending = Collapse::default();
    let mut decorations = Vec::new();
    let mut ignored_borders = 0_usize;
    let mut not_enclosing = 0_usize;
    let mut trimmed_leading = (0_usize, 0.0_f32);
    let mut escaped = crate::layout::contracts::Escapes::default();
    let mut overrun = (0_usize, 0.0_f32);
    let mut refitted = (0_usize, 1.0_f32);
    let mut sink = Sink {
        units: &mut units,
        usable,
        decorations: &mut decorations,
        ignored_borders: &mut ignored_borders,
        not_enclosing: &mut not_enclosing,
        trimmed_leading: &mut trimmed_leading,
        sheet,
        escaped: &mut escaped,
        overrun: &mut overrun,
        refitted: &mut refitted,
    };

    let canvas = boxes.canvas;
    place(
        boxes.root,
        measure,
        &mut cursor,
        &mut pending,
        &mut sink,
        OwnBorder::Emit,
    )?;

    pending.flush_into(&mut cursor);

    Ok(FragmentTree {
        units,
        canvas,
        decorations,
        borders_on_rows: ignored_borders,
        borders_not_enclosing: not_enclosing,
        trimmed_leading,
        escaped,
        overrun,
        refitted,
        content_height: Length::try_new(cursor, "computing the content height")?,
    })
}

#[derive(Clone, Copy)]
pub(super) struct Extent {
    pub(super) first_unit: usize,

    pub(super) top: Option<f32>,

    pub(super) empty_top: f32,
    pub(super) bottom: f32,

    pub(super) left: f32,

    pub(super) width: f32,
}

pub(super) fn record_border(
    border: crate::contracts::BorderEdges,
    surface: crate::contracts::Surface,
    paints: bool,
    sink: &mut Sink<'_>,
    own: OwnBorder,
    extent: Extent,
) -> Result<(), Error> {
    let anything = border.any() || paints;
    if !anything || own == OwnBorder::CallerRecords {
        return Ok(());
    }

    let members: Vec<usize> = (extent.first_unit..sink.units.len()).collect();

    let members = if members.is_empty() {
        let top = extent.top.unwrap_or(extent.empty_top);
        let anchor = FlowUnit::box_anchor(
            Length::try_new(top, "an empty box's anchor top")?,
            Length::try_new(
                (extent.bottom - top).max(0.0),
                "an empty box's anchor height",
            )?,
            crate::contracts::Band::of(surface, extent.left, extent.width)?,
        )?;
        let index = sink.units.len();

        match sink.units.push(anchor) {
            Ok(()) => vec![index],

            Err(Error::FlowOutOfOrder { .. }) => return Ok(()),

            Err(other) => return Err(other),
        }
    } else {
        members
    };

    if !border.any() {
        return Ok(());
    }
    let top = match extent.top {
        Some(value) => Some(Length::try_new(value, "a decoration's top")?),
        None => None,
    };

    match Decoration::new(
        top,
        Length::try_new(extent.bottom, "a decoration's bottom")?,
        Length::try_new(extent.left, "a decoration's left")?,
        Length::try_new(extent.width, "a decoration's width")?,
        border,
        members,
        sink.units,
    ) {
        Ok(decoration) => sink.decorations.push(decoration),
        Err(Error::DecorationExtentDisagreesWithMembers { .. }) => {
            *sink.not_enclosing = sink.not_enclosing.saturating_add(1);
        }

        Err(other) => return Err(other),
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one inward/outward box walk owns the paired box-model geometry; splitting it would duplicate or conceal that lifecycle"
)]
pub(super) fn place(
    node: BoxNode,
    outer: crate::contracts::UsableWidth,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut Sink<'_>,
    own: OwnBorder,
) -> Result<(), Error> {
    let node_left_inset = node.border.left.used_width() + node.padding.left.get();
    let node_right_inset = node.border.right.used_width() + node.padding.right.get();
    let left = outer.left() + node.margin.left.get() + node_left_inset;
    let filled = outer.extent()
        - node.margin.left.get()
        - node.margin.right.get()
        - node_left_inset
        - node_right_inset;

    let declared_content = |declared: f32| match node.box_sizing {
        crate::contracts::BoxSizing::ContentBox => declared,
        crate::contracts::BoxSizing::BorderBox => {
            (declared - node_left_inset - node_right_inset).max(0.0)
        }
    };
    let available = match node.width {
        crate::contracts::Width::Auto => filled,
        crate::contracts::Width::Fixed(spacing) => {
            declared_content(spacing.resolve(node.font_size)?.get())
        }

        crate::contracts::Width::Percent(fraction) => declared_content(outer.extent() * fraction),
    };

    let painted_left = left - node_left_inset;
    let painted_extent = available + node_left_inset + node_right_inset;

    let paints = node.paints
        || node.border.left.used_width() > 0.0
        || node.border.right.used_width() > 0.0
        || node.border.top.used_width() > 0.0
        || node.border.bottom.used_width() > 0.0;
    record_if_off_the_sheet(sink, painted_left, painted_extent, node.width, paints);

    let node_paints = node.paints;
    let node_style = BoxStyle {
        link: node.link.clone(),
        face: node.face,
        authored_face: node.authored_face.clone(),
        font_size: node.font_size,
        line_height: node.line_height,
        text_align: node.text_align,
        colour: node.colour,
        surface: node.surface,
        band: crate::contracts::Band::of(node.surface, painted_left, painted_extent)?,
    };

    let first_unit = sink.units.len();

    let own_border = node.border;

    pending.absorb(node.margin.top.get());

    let border_box_top = if node.border.top.used_width() > 0.0 || node.padding.top.get() > 0.0 {
        pending.flush_into(cursor);
        let top = *cursor;
        *cursor += node.border.top.used_width() + node.padding.top.get();
        Some(top)
    } else {
        None
    };

    let mut inline: Vec<InlinePiece> = Vec::new();

    let bottom_edge = node.border.bottom.used_width() + node.padding.bottom.get();
    let node_display = node.display;
    let flex_container = node.flex_container;
    let BoxNode {
        content, children, ..
    } = node;

    #[allow(
        clippy::match_same_arms,
        reason = "distinct cases whose bodies coincide; merging couples them"
    )]
    match content {
        InlineContent::Text(text) => inline.push(InlinePiece::Run(Run::of(&node_style, text))),

        InlineContent::Placed(image) => {
            inline.push(InlinePiece::Image(image, node_style.link.clone()));
        }

        InlineContent::Replaced(_) => {}

        InlineContent::ForcedBreak => {}
        InlineContent::Empty => {}
    }

    if matches!(node_display, Display::Flex | Display::InlineFlex) {
        emit_pieces(
            &mut inline,
            &node_style,
            left,
            available,
            cursor,
            pending,
            sink,
        )?;
        match flex_container.direction {
            crate::contracts::FlexDirection::Row => super::flex::place_row(
                children,
                flex_container,
                node_style.band,
                left,
                available,
                cursor,
                pending,
                sink,
            )?,
            crate::contracts::FlexDirection::Column => super::flex::place_column(
                children,
                flex_container,
                left,
                available,
                cursor,
                pending,
                sink,
            )?,
        }
    } else {
        place_children(
            children,
            &mut inline,
            &node_style,
            left,
            available,
            cursor,
            pending,
            sink,
        )?;
    }
    emit_pieces(
        &mut inline,
        &node_style,
        left,
        available,
        cursor,
        pending,
        sink,
    )?;

    let empty_top = close_bottom_edge(bottom_edge, cursor, pending);
    record_border(
        own_border,
        node_style.surface,
        node_paints,
        sink,
        own,
        Extent {
            first_unit,
            top: border_box_top,
            empty_top,
            bottom: *cursor,

            left: left - node_left_inset,
            width: available + node_left_inset + node_right_inset,
        },
    )?;

    pending.absorb(node.margin.bottom.get());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn place_children(
    children: Vec<BoxNode>,
    inline: &mut Vec<InlinePiece>,
    node_style: &BoxStyle,
    left: f32,
    available: f32,
    cursor: &mut f32,
    pending: &mut Collapse,
    sink: &mut Sink<'_>,
) -> Result<(), Error> {
    for child in children {
        match child.display {
            _ if child.content == InlineContent::ForcedBreak => {
                inline.push(InlinePiece::Break);
            }
            Display::Inline => gather(child, inline),
            Display::InlineFlex => inline.push(InlinePiece::Flex(Box::new(child))),

            Display::Table => {
                emit_pieces(inline, node_style, left, available, cursor, pending, sink)?;
                place_table(child, left, available, cursor, pending, sink)?;
            }

            Display::Block | Display::Flex | Display::RowGroup | Display::Row | Display::Cell => {
                emit_pieces(inline, node_style, left, available, cursor, pending, sink)?;
                place(
                    child,
                    crate::contracts::UsableWidth::span(left, available)?,
                    cursor,
                    pending,
                    sink,
                    OwnBorder::Emit,
                )?;
            }
        }
    }
    Ok(())
}

fn close_bottom_edge(bottom_edge: f32, cursor: &mut f32, pending: &mut Collapse) -> f32 {
    if bottom_edge > 0.0 {
        pending.flush_into(cursor);
        let top = *cursor;
        *cursor += bottom_edge;
        return top;
    }
    *cursor
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Collapse {
    largest_positive: f32,
    most_negative: f32,
}

impl Collapse {
    pub(super) fn absorb(&mut self, margin: f32) {
        if margin >= 0.0 {
            self.largest_positive = self.largest_positive.max(margin);
        } else {
            self.most_negative = self.most_negative.min(margin);
        }
    }

    const fn value(self) -> f32 {
        self.largest_positive + self.most_negative
    }

    pub(super) fn flush_into(&mut self, cursor: &mut f32) {
        *cursor += self.value();
        *self = Self::default();
    }
}

fn record_if_off_the_sheet(
    sink: &mut Sink<'_>,
    painted_left: f32,
    painted_extent: f32,
    width: crate::contracts::Width,
    paints: bool,
) {
    if !paints {
        return;
    }

    let declared = matches!(
        width,
        crate::contracts::Width::Fixed(_) | crate::contracts::Width::Percent(_)
    );
    sink.escaped.record(
        painted_left,
        painted_left + painted_extent,
        sink.sheet,
        declared,
    );
}
