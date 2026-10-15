use crate::contracts::{FontSize, Length, PageSize, Rgb};

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct PlacedFragment {
    pub text: String,

    pub clip: Option<PageBand>,

    pub continued: bool,
    pub x: Length,

    pub y: Length,

    pub line_height: Length,
    pub font_size: FontSize,
    pub face: crate::metrics::Face,
    pub(crate) positioned: Option<crate::font::PositionedRun>,

    pub colour: Rgb,
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Page {
    pub fragments: Vec<PlacedFragment>,

    pub(crate) furniture: Vec<crate::contracts::PlacedFurniture>,

    pub(crate) images: Vec<PlacedPicture>,

    pub(crate) links: Vec<PlacedLink>,

    pub(crate) bands: Vec<PlacedRect>,

    pub(crate) decorations: Vec<PlacedRect>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlacedLink {
    pub(crate) target: crate::contracts::LinkTarget,
    pub(crate) x: Length,
    pub(crate) y: Length,
    pub(crate) width: Length,
    pub(crate) height: Length,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlacedPicture {
    pub(crate) asset: usize,
    pub(crate) x: Length,

    pub(crate) y: Length,
    pub(crate) width: Length,
    pub(crate) height: Length,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub(crate) struct PlacedRect {
    pub(crate) x: Length,
    pub(crate) y: Length,
    pub(crate) width: Length,
    pub(crate) height: Length,
    pub(crate) colour: Rgb,
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct Pages {
    pub pages: Vec<Page>,
    pub page_size: PageSize,

    pub canvas: Option<Rgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PagePiece {
    page: usize,
}

impl PagePiece {
    pub(crate) fn reserving(page: usize) -> Self {
        Self { page }
    }

    pub(crate) fn page(self) -> usize {
        self.page
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PieceMap {
    by_unit: Vec<Vec<PagePiece>>,
}

impl PieceMap {
    pub(crate) fn for_units(count: usize) -> Self {
        Self {
            by_unit: vec![Vec::new(); count],
        }
    }

    pub(crate) fn record(&mut self, unit: usize, piece: PagePiece) {
        if let Some(pieces) = self.by_unit.get_mut(unit) {
            pieces.push(piece);
        }
    }

    pub(crate) fn pieces(&self, unit: usize) -> &[PagePiece] {
        self.by_unit.get(unit).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn sole_page(&self, unit: usize) -> Option<usize> {
        let pieces = self.pieces(unit);
        let first = pieces.first()?.page();
        pieces
            .iter()
            .all(|piece| piece.page() == first)
            .then_some(first)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PageBand {
    pub top: Length,
    pub height: Length,
}
