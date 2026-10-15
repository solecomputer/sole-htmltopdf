#![forbid(unsafe_code)]

mod acquire;
mod boxes;
mod cascade;
mod compose;
mod contracts;
mod core;
mod css;
mod display;
mod dom;
mod emulation;
mod encoding;
mod font;
mod image;
mod inputs;
mod intent;
mod layout;
mod metrics;
mod paginate;
mod parse;
mod pdf;
mod selector;
mod space;

pub use contracts::{
    Axis, BUILT_IN, BandSource, BoxContent, BoxNode, BoxTree, Bundle, Colours, ComputedStyle,
    DEFAULT_MAX_DEPTH, DEFAULT_MAX_INPUT_BYTES, DEFAULT_MAX_NODES, DEFAULT_PAGE_MARGIN_POINTS,
    DisplayItem, DisplayList, DisplayPage, Doctype, DocumentBase, DocumentFacts, Dom, Edges, Error,
    FlowFragment, FontSize, FragmentTree, Furniture, HostError, HtmlQuirksMode, ImageRequest,
    Images, InputRef, Length, MarginBand, MarginRule, Medium, Page, PageOrientation, PageSize,
    PageToken, Pages, PdfBytes, Piece, PlacedFragment, PositionedRun, RenderHost, RenderMode,
    RenderOptions, RenderOutcome, Requester, Rgb, Slot, SmartShrinking, StyledTree, StylesheetRef,
    SubdocumentId, TemplateNotice, VerticalPlacement, Warning, Zoom, builtin_token, parse_template,
    resolve_token,
};

pub use core::render;
pub use encoding::Encoding;
pub use metrics::Face;
