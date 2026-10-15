use super::HostError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl std::fmt::Display for Axis {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = match *self {
            Self::Horizontal => "width",
            Self::Vertical => "height",
        };
        formatter.write_str(word)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("no input given")]
    NoInput,

    #[error("input {origin} is not valid UTF-8 at byte {position}")]
    NotUtf8 { origin: String, position: usize },

    #[error(
        "{origin} is a network address and this version fetches nothing: \
         download it and pass the local file"
    )]
    NetworkInput { origin: String },

    #[error("input could not be read: {0}")]
    Host(#[from] HostError),

    #[error("no input could be read ({} attempted): {}", causes.len(), causes.join("; "))]
    AllInputsFailed { causes: Vec<String> },

    #[error("page size is invalid: {width} x {height} (must be finite and positive)")]
    InvalidPageSize { width: f32, height: f32 },

    #[error("font size is invalid: {value} (must be finite and positive)")]
    InvalidFontSize { value: f32 },

    #[error("unknown page size: {name}")]
    UnknownPageSize { name: String },

    #[error("computed a non-finite measurement while {context}")]
    NonFiniteMeasurement { context: &'static str },

    #[error("document produced no pages")]
    NoPages,

    #[error("flow unit ends at {next}pt, above its predecessor's bottom at {previous}pt")]
    FlowOutOfOrder { previous: f32, next: f32 },

    #[error("a block decoration was built with no member flow units")]
    DecorationWithoutMembers,

    #[error(
        "a decoration spanning {top}..{bottom} does not enclose the units it \
         names, which span {span_top}..{span_bottom}"
    )]
    DecorationExtentDisagreesWithMembers {
        top: f32,
        bottom: f32,
        span_top: f32,
        span_bottom: f32,
    },

    #[error("colour channel {channel} is outside 0.0..=1.0")]
    ColourOutOfRange { channel: f32 },

    #[error("flow unit has a negative height of {height}pt")]
    NegativeFlowExtent { height: f32 },

    #[error("document would exceed the {limit}-page limit")]
    PageLimitExceeded { limit: usize },

    #[error("the page margins ({margins}pt) leave no room across the {extent}pt page {axis}")]
    PageMarginsExceedPage {
        axis: Axis,
        extent: f32,
        margins: f32,
    },

    #[error("input exceeds the {limit}-byte limit")]
    InputTooLarge { limit: usize },

    #[error("required input {origin} exceeds the aggregate {limit}-byte input limit")]
    RequiredInputTooLarge { origin: String, limit: usize },

    #[error("a rendering scale of {value} is not a finite positive factor")]
    InvalidScale { value: f32 },
    #[error("document exceeds the {limit} limit ({seen} nodes seen before parsing stopped)")]
    DocumentTooComplex { limit: &'static str, seen: usize },

    #[error("this version cannot embed {what}")]
    ImageRefused { what: &'static str },

    #[error("{edge} asks for {needed}pt of a margin that leaves {available}pt")]
    FurnitureDoesNotFit {
        edge: &'static str,
        needed: f32,
        available: f32,
    },
}
