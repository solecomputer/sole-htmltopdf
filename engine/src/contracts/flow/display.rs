use crate::contracts::{FontSize, Length, PageSize, Rgb};

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum DisplayItem {
    Text {
        text: String,
        x: Length,
        y: Length,
        font_size: FontSize,
        face: crate::metrics::Face,
        positioned: Option<crate::contracts::PositionedRun>,

        colour: Rgb,

        clip: Option<PdfBand>,

        continued: bool,
    },

    Image {
        asset: usize,
        x: Length,
        y: Length,
        width: Length,
        height: Length,
    },

    Rect {
        x: Length,
        y: Length,
        width: Length,
        height: Length,
        colour: Rgb,
    },
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct DisplayPage {
    pub items: Vec<DisplayItem>,
    pub(crate) links: Vec<PdfLink>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PdfLink {
    pub(crate) target: crate::contracts::LinkTarget,
    pub(crate) x: Length,
    pub(crate) y: Length,
    pub(crate) width: Length,
    pub(crate) height: Length,
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct DisplayList {
    pub pages: Vec<DisplayPage>,
    pub page_size: PageSize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PdfBand {
    pub y: Length,
    pub height: Length,
}
