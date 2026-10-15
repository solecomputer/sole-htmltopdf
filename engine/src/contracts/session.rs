use super::{Edges, Length, PageSize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputRef {
    Inline { bytes: Vec<u8>, origin: String },

    Path(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StylesheetRef {
    Inline { bytes: Vec<u8>, origin: String },
    Path(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Requester {
    Caller,
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Zoom(f32);

impl Zoom {
    pub const NONE: Self = Self(1.0);

    pub const SMALLEST: f32 = 0.1;

    pub const LARGEST: f32 = 10.0;

    #[must_use = "a validated zoom is the only way to carry a factor across the crate boundary"]
    pub fn try_new(factor: f32) -> Result<Self, super::Error> {
        if factor.is_finite() && (Self::SMALLEST..=Self::LARGEST).contains(&factor) {
            Ok(Self(factor))
        } else {
            Err(super::Error::InvalidScale { value: factor })
        }
    }

    #[must_use]
    pub fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Requested<T> {
    Fallback(T),
    Caller(T),
}

impl<T: Copy> Requested<T> {
    fn value(self) -> T {
        match self {
            Self::Fallback(value) | Self::Caller(value) => value,
        }
    }

    fn author_or(self, authored: Option<T>) -> (T, bool) {
        match (self, authored) {
            (Self::Fallback(_), Some(value)) => (value, true),
            _ => (self.value(), false),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PageRequest {
    size: Requested<PageSize>,
    top: Requested<Length>,
    right: Requested<Length>,
    bottom: Requested<Length>,
    left: Requested<Length>,
    orientation: Option<super::PageOrientation>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PageGeometry {
    pub(crate) size: PageSize,
    pub(crate) margin: Edges,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RenderOptions {
    page: PageRequest,

    pub max_input_bytes: usize,

    pub max_nodes: usize,

    pub max_depth: usize,

    pub title: Option<String>,

    pub encoding: Option<crate::encoding::Encoding>,

    pub mode: super::RenderMode,

    pub zoom: Zoom,

    pub colours: super::Colours,

    pub medium: super::Medium,

    pub images: super::Images,

    pub author_stylesheets: Vec<StylesheetRef>,

    pub asset_root: Option<PathBuf>,

    pub furniture: super::Furniture,

    pub header_html: Option<super::BandSource>,

    pub footer_html: Option<super::BandSource>,

    pub furniture_facts: super::DocumentFacts,

    pub furniture_bindings: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Images {
    #[default]
    On,

    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Medium {
    #[default]
    Default,

    Print,

    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Evaluated {
    Print,
    Screen,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Colours {
    #[default]
    On,

    Off,
}

pub const DEFAULT_PAGE_MARGIN_POINTS: f32 = 28.35;

pub const DEFAULT_MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

pub const DEFAULT_MAX_NODES: usize = 2_000_000;

pub const DEFAULT_MAX_DEPTH: usize = 512;

impl RenderOptions {
    #[must_use]
    pub fn new(page_size: PageSize) -> Self {
        let margin = Length::from_finite(DEFAULT_PAGE_MARGIN_POINTS);
        Self {
            page: PageRequest {
                size: Requested::Caller(page_size),
                top: Requested::Fallback(margin),
                right: Requested::Fallback(margin),
                bottom: Requested::Fallback(margin),
                left: Requested::Fallback(margin),
                orientation: None,
            },
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
            max_nodes: DEFAULT_MAX_NODES,
            max_depth: DEFAULT_MAX_DEPTH,
            title: None,
            encoding: None,
            colours: Colours::On,

            medium: Medium::Default,
            images: Images::On,
            author_stylesheets: Vec::new(),
            asset_root: None,
            furniture: super::Furniture::default(),
            header_html: None,
            footer_html: None,
            furniture_facts: super::DocumentFacts::default(),
            furniture_bindings: Vec::new(),
            mode: super::RenderMode::Modern,
            zoom: Zoom::NONE,
        }
    }

    #[must_use]
    pub fn document_defaults() -> Self {
        let mut options = Self::new(PageSize::a4());
        options.page.size = Requested::Fallback(PageSize::a4());
        options
    }

    #[must_use]
    pub fn page_size(&self) -> PageSize {
        self.page.size.value()
    }

    #[must_use]
    pub fn page_margin(&self) -> Edges {
        Edges {
            top: self.page.top.value(),
            right: self.page.right.value(),
            bottom: self.page.bottom.value(),
            left: self.page.left.value(),
        }
    }

    pub fn set_page_size(&mut self, page_size: PageSize) {
        self.page.size = Requested::Caller(page_size);
    }

    pub fn set_page_margin(&mut self, margin: Edges) {
        self.page.top = Requested::Caller(margin.top);
        self.page.right = Requested::Caller(margin.right);
        self.page.bottom = Requested::Caller(margin.bottom);
        self.page.left = Requested::Caller(margin.left);
    }

    pub fn set_page_margin_top(&mut self, value: Length) {
        self.page.top = Requested::Caller(value);
    }

    pub fn set_page_margin_right(&mut self, value: Length) {
        self.page.right = Requested::Caller(value);
    }

    pub fn set_page_margin_bottom(&mut self, value: Length) {
        self.page.bottom = Requested::Caller(value);
    }

    pub fn set_page_margin_left(&mut self, value: Length) {
        self.page.left = Requested::Caller(value);
    }

    pub fn set_page_orientation(&mut self, orientation: super::PageOrientation) {
        self.page.orientation = Some(orientation);
    }

    pub(crate) fn baseline_page_geometry(&self) -> PageGeometry {
        self.orient(PageGeometry {
            size: self.page_size(),
            margin: self.page_margin(),
        })
    }

    pub(crate) fn authored_page_geometry(
        &self,
        authored: super::AuthorPage,
    ) -> (PageGeometry, bool) {
        let authored_size = match authored.size() {
            Some(super::AuthorSize::Page(size)) => Some(size),
            Some(super::AuthorSize::Auto) | None => None,
        };
        let (size, size_from_author) = self.page.size.author_or(authored_size);
        let (top, top_from_author) = self
            .page
            .top
            .author_or(authored.margin(super::PageEdge::Top));
        let (right, right_from_author) = self
            .page
            .right
            .author_or(authored.margin(super::PageEdge::Right));
        let (bottom, bottom_from_author) = self
            .page
            .bottom
            .author_or(authored.margin(super::PageEdge::Bottom));
        let (left, left_from_author) = self
            .page
            .left
            .author_or(authored.margin(super::PageEdge::Left));
        (
            self.orient(PageGeometry {
                size,
                margin: Edges {
                    top,
                    right,
                    bottom,
                    left,
                },
            }),
            size_from_author
                || top_from_author
                || right_from_author
                || bottom_from_author
                || left_from_author,
        )
    }

    fn orient(&self, mut geometry: PageGeometry) -> PageGeometry {
        let width = geometry.size.width().get();
        let height = geometry.size.height().get();
        let turn = match self.page.orientation {
            Some(super::PageOrientation::Portrait) => width > height,
            Some(super::PageOrientation::Landscape) => width < height,
            None => false,
        };
        if turn {
            geometry.size = geometry.size.landscape();
        }
        geometry
    }

    #[must_use]
    pub fn with_max_input_bytes(mut self, limit: usize) -> Self {
        self.max_input_bytes = limit;
        self
    }
}

pub trait RenderHost {
    fn read_file(
        &self,
        path: &Path,
        max_bytes: usize,
        requester: Requester,
    ) -> Result<Vec<u8>, HostError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum HostError {
    #[error("not found: {path}")]
    NotFound { path: String },

    #[error("access denied by policy: {path}")]
    Denied { path: String },

    #[error("local document access is disabled: {path}")]
    AccessDisabled { path: String },

    #[error("document resource is outside the permitted root {root}: {path}")]
    OutsideRoot { path: String, root: String },

    #[error("unreadable: {path}: {detail}")]
    Unreadable { path: String, detail: String },

    #[error("{path} exceeds the {limit}-byte limit")]
    TooLarge { path: String, limit: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DocumentBase {
    Directory(PathBuf),

    None,
}
