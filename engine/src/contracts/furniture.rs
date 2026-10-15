use super::{Length, Rgb};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Furniture {
    pub top: Option<MarginBand>,
    pub bottom: Option<MarginBand>,
}

impl Furniture {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.bottom.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarginBand {
    pub content: [BoxContent; 3],

    pub rule: Option<Rule>,
    pub face: crate::metrics::Face,
    pub font_size: super::FontSize,

    pub colour: Rgb,

    pub placement: VerticalPlacement,

    pub spacing: Length,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoxContent {
    Slot(Slot),

    Document(SubdocumentId),
}

impl Default for BoxContent {
    fn default() -> Self {
        Self::Slot(Slot::default())
    }
}

impl BoxContent {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Slot(slot) => slot.is_empty(),

            Self::Document(_) => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubdocumentId(usize);

#[derive(Debug, Default)]
pub(crate) struct Subdocuments {
    compiled: Vec<super::BoxTree>,
}

impl Subdocuments {
    pub(crate) fn add(&mut self, compiled: super::BoxTree) -> SubdocumentId {
        self.compiled.push(compiled);
        SubdocumentId(self.compiled.len() - 1)
    }

    pub(crate) fn get(&self, id: SubdocumentId) -> Option<&super::BoxTree> {
        self.compiled.get(id.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateNotice {
    NotComputed(&'static str),

    SourceHasNoName,

    RebindRefused(String),
}

#[must_use]
pub fn resolve_token(
    name: &str,
    bindings: &[(String, String)],
    named_source: bool,
) -> Option<(PageToken, Option<TemplateNotice>)> {
    if let Some((_, value)) = bindings.iter().find(|(bound, _)| bound == name) {
        if BUILT_IN.contains(&name) {
            return builtin_token(name, named_source)
                .map(|(token, _)| (token, Some(TemplateNotice::RebindRefused(name.to_owned()))));
        }
        return Some((PageToken::Replaced(value.clone()), None));
    }
    builtin_token(name, named_source)
}

pub const BUILT_IN: [&str; 13] = [
    "page",
    "topage",
    "frompage",
    "title",
    "doctitle",
    "date",
    "isodate",
    "time",
    "webpage",
    "section",
    "subsection",
    "sitepage",
    "sitepages",
];

#[must_use]
pub fn builtin_token(
    name: &str,
    named_source: bool,
) -> Option<(PageToken, Option<TemplateNotice>)> {
    let plain = |token| Some((token, None));
    match name {
        "page" => plain(PageToken::Page),
        "topage" => plain(PageToken::Pages),
        "title" | "doctitle" => plain(PageToken::Title),
        "date" | "isodate" => plain(PageToken::Date),
        "time" => plain(PageToken::Time),
        "webpage" => {
            if named_source {
                plain(PageToken::Source)
            } else {
                Some((
                    PageToken::Unanswerable("webpage"),
                    Some(TemplateNotice::SourceHasNoName),
                ))
            }
        }
        "frompage" | "section" | "subsection" | "sitepage" | "sitepages" => {
            let named = match name {
                "frompage" => "frompage",
                "section" => "section",
                "subsection" => "subsection",
                "sitepage" => "sitepage",
                _ => "sitepages",
            };
            Some((
                PageToken::Unanswerable(named),
                Some(TemplateNotice::NotComputed(named)),
            ))
        }
        _ => None,
    }
}

pub fn parse_template(text: &str, mut lookup: impl FnMut(&str) -> Option<PageToken>) -> Slot {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let (before, tail) = rest.split_at(open);
        literal.push_str(before);
        let Some(close) = tail.find(']') else {
            literal.push_str(tail);
            rest = "";
            break;
        };
        let name = tail.get(1..close).unwrap_or_default();
        match lookup(name) {
            Some(token) => {
                if !literal.is_empty() {
                    pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                }
                pieces.push(Piece::Token(token));
            }

            None => literal.push_str(tail.get(..=close).unwrap_or_default()),
        }
        rest = tail.get(close + 1..).unwrap_or_default();
    }
    literal.push_str(rest);
    if !literal.is_empty() {
        pieces.push(Piece::Literal(literal));
    }
    Slot::new(pieces)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BandSource {
    Path(std::path::PathBuf),

    Markup(String),
}

impl MarginBand {
    #[must_use]
    pub fn for_subdocument(id: SubdocumentId) -> Self {
        Self {
            content: [
                BoxContent::default(),
                BoxContent::Document(id),
                BoxContent::default(),
            ],
            rule: None,
            placement: VerticalPlacement::InnerEdge,
            face: crate::metrics::Face::Helvetica,
            font_size: super::FontSize::FURNITURE_DEFAULT,
            colour: super::Rgb::BLACK,

            spacing: super::Length::from_finite(0.0),
        }
    }

    pub fn slots(&self) -> impl Iterator<Item = &Slot> {
        self.content.iter().filter_map(|box_| match box_ {
            BoxContent::Slot(slot) => Some(slot),
            BoxContent::Document(_) => None,
        })
    }

    pub fn slot_mut(&mut self, which: usize) -> Option<&mut Slot> {
        match self.content.get_mut(which) {
            Some(BoxContent::Slot(slot)) => Some(slot),
            Some(BoxContent::Document(_)) | None => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    pub thickness: Length,
    pub colour: Rgb,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Slot {
    pieces: Vec<Piece>,
}

impl Slot {
    #[must_use]
    pub fn new(pieces: Vec<Piece>) -> Self {
        Self { pieces }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub(crate) fn tokens(&self) -> impl Iterator<Item = &PageToken> {
        self.pieces.iter().filter_map(|piece| match piece {
            Piece::Token(token) => Some(token),
            Piece::Literal(_) => None,
        })
    }

    #[must_use]
    pub fn stamp(&self, facts: &PageFacts) -> String {
        let mut out = String::new();
        for piece in &self.pieces {
            match piece {
                Piece::Literal(text) => out.push_str(text),
                Piece::Token(token) => out.push_str(&token.resolve(facts)),
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Literal(String),
    Token(PageToken),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageToken {
    Page,

    Pages,

    Title,

    Source,

    Date,

    Time,

    Unanswerable(&'static str),

    Replaced(String),
}

impl PageToken {
    fn resolve(&self, facts: &PageFacts) -> String {
        match self {
            Self::Page => facts.ordinal.to_string(),
            Self::Pages => facts.total.to_string(),
            Self::Title => facts.title.clone().unwrap_or_default(),
            Self::Source => facts.source.clone().unwrap_or_default(),
            Self::Date => facts.date.clone().unwrap_or_default(),
            Self::Time => facts.time.clone().unwrap_or_default(),
            Self::Unanswerable(_) => String::new(),
            Self::Replaced(value) => value.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageFacts {
    ordinal: usize,
    total: usize,
    title: Option<String>,
    source: Option<String>,
    date: Option<String>,
    time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocumentFacts {
    pub title: Option<String>,
    pub source: Option<String>,

    pub date: Option<String>,

    pub time: Option<String>,
}

impl PageFacts {
    #[must_use]
    pub fn of(pages: &super::Pages, index: usize, document: &DocumentFacts) -> Self {
        Self {
            ordinal: index.saturating_add(1),
            total: pages.pages.len(),
            title: document.title.clone(),
            source: document.source.clone(),
            date: document.date.clone(),
            time: document.time.clone(),
        }
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    pub(crate) const fn has_named_source(&self) -> bool {
        self.source.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PlacedFurniture {
    Text {
        text: String,
        x: Length,

        baseline: Length,
        font_size: super::FontSize,
        face: crate::metrics::Face,
        positioned: Option<crate::contracts::PositionedRun>,
        colour: Rgb,
    },
    Rule {
        x: Length,

        y: Length,
        width: Length,
        height: Length,
        colour: Rgb,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub(crate) struct FrameRect {
    pub(crate) x: Length,

    pub(crate) y: Length,
    pub(crate) width: Length,
    pub(crate) height: Length,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub(crate) struct PageFrame {
    pub(crate) media: FrameRect,

    pub(crate) content: FrameRect,
    pub(crate) header: FrameRect,
    pub(crate) footer: FrameRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalPlacement {
    InnerEdge,

    Centred,
}

impl PageFrame {
    pub(crate) fn derive(
        page: super::PageSize,
        margin: super::Edges,
        top_spacing: Option<Length>,
        bottom_spacing: Option<Length>,
    ) -> Result<Self, super::Error> {
        let zero = Length::from_finite(0.0);

        let fits = |spacing: Option<Length>, available: Length, edge: &'static str| {
            let Some(spacing) = spacing else {
                return Ok(());
            };
            if available.get() > 0.0 && spacing.get() >= available.get() {
                Err(super::Error::FurnitureDoesNotFit {
                    edge,
                    needed: spacing.get(),
                    available: available.get(),
                })
            } else {
                Ok(())
            }
        };
        fits(top_spacing, margin.top, "--header-spacing")?;
        fits(bottom_spacing, margin.bottom, "--footer-spacing")?;

        let top_spacing = top_spacing.unwrap_or(zero);
        let bottom_spacing = bottom_spacing.unwrap_or(zero);

        let width = page.width();
        let inner_width = Length::try_new(
            (width.get() - margin.left.get() - margin.right.get()).max(0.0),
            "the content band's width",
        )?;
        let media = FrameRect {
            x: zero,
            y: zero,
            width,
            height: page.height(),
        };
        let content = FrameRect {
            x: margin.left,
            y: margin.top,
            width: inner_width,
            height: Length::try_new(
                (page.height().get() - margin.top.get() - margin.bottom.get()).max(0.0),
                "the content band's height",
            )?,
        };

        let header = FrameRect {
            x: margin.left,
            y: zero,
            width: inner_width,
            height: Length::try_new(
                (margin.top.get() - top_spacing.get()).max(0.0),
                "the header band's height",
            )?,
        };

        let footer_top = page.height().get() - margin.bottom.get() + bottom_spacing.get();
        let footer = FrameRect {
            x: margin.left,
            y: Length::try_new(footer_top, "the footer band's top")?,
            width: inner_width,
            height: Length::try_new(
                (margin.bottom.get() - bottom_spacing.get()).max(0.0),
                "the footer band's height",
            )?,
        };
        Ok(Self {
            media,
            content,
            header,
            footer,
        })
    }
}

impl FrameRect {
    #[must_use]
    pub(crate) fn baseline_for(
        &self,
        edge: Edge,
        placement: VerticalPlacement,
        face: crate::metrics::Face,
        font_size: f32,
        line_height: f32,
    ) -> f32 {
        let (ascent, descent) = face.vertical();
        let ascent = f32::from(ascent) / 1000.0 * font_size;
        let descent = f32::from(descent) / 1000.0 * font_size;
        let bottom = self.y.get() + self.height.get();
        match placement {
            VerticalPlacement::Centred => {
                let top = self.y.get() + (self.height.get() - line_height) / 2.0;
                top + face.baseline_offset(font_size, line_height)
            }
            VerticalPlacement::InnerEdge => match edge {
                Edge::Top => bottom - descent,

                Edge::Bottom => self.y.get() + ascent,
            },
        }
    }
}
