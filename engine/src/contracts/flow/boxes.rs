use crate::contracts::{
    BorderEdges, Display, Edges, FlexAlign, FlexDirection, FlexFactor, FlexJustify, FontSize,
    Length, Rgb, Surface, TextAlign, Width,
};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BoxNode {
    pub(crate) link: Option<super::super::LinkTarget>,

    pub content: InlineContent,
    pub font_size: FontSize,
    pub margin: Edges,
    pub padding: Edges,

    pub border: BorderEdges,
    pub line_height: Length,

    pub width: Width,
    pub(crate) box_sizing: crate::contracts::BoxSizing,
    pub display: Display,
    pub(crate) flex_container: BoxFlexContainer,
    pub(crate) flex_item: BoxFlexItem,
    pub text_align: TextAlign,

    pub face: crate::metrics::Face,
    pub(crate) authored_face: Option<crate::font::AuthoredFace>,

    pub colour: Rgb,

    pub surface: Surface,

    pub paints: bool,
    pub children: Vec<BoxNode>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoxFlexContainer {
    pub(crate) direction: FlexDirection,
    pub(crate) justify: FlexJustify,
    pub(crate) align_items: FlexAlign,
    pub(crate) row_gap: Length,
    pub(crate) column_gap: Length,
}

impl Default for BoxFlexContainer {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            justify: FlexJustify::Start,
            align_items: FlexAlign::Stretch,
            row_gap: Length::ZERO,
            column_gap: Length::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BoxFlexItem {
    pub(crate) grow: FlexFactor,
    pub(crate) shrink: FlexFactor,
    pub(crate) basis: BoxFlexBasis,
    pub(crate) align_self: Option<FlexAlign>,
}

impl Default for BoxFlexItem {
    fn default() -> Self {
        Self {
            grow: FlexFactor::ZERO,
            shrink: FlexFactor::ONE,
            basis: BoxFlexBasis::Auto,
            align_self: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BoxFlexBasis {
    Auto,
    Content,
    Fixed(Length),
    Percent(f32),
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BoxTree {
    pub root: BoxNode,

    pub canvas: Option<Rgb>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InlineContent {
    Empty,

    Text(String),

    ForcedBreak,

    Replaced(Box<crate::contracts::ImageRequest>),

    Placed(PlacedImage),
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PlacedImage {
    pub asset: usize,

    pub width: f32,
    pub height: f32,

    pub acquired_scale: f32,
}
