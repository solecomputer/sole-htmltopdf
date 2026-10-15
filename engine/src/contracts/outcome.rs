#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfBytes(Vec<u8>);

impl PdfBytes {
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Warning {
    #[error(
        "{count} authored font source(s) were refused (for example: {}); the document kept its visible fallback",
        examples.join(", ")
    )]
    AuthoredFontsRefused { count: usize, examples: Vec<String> },

    #[error(
        "{count} authored font face(s) were ignored because embedded authored fonts are modern-only"
    )]
    AuthoredFontsIgnoredInLegacy { count: usize },

    #[error(
        "{count} furniture-local @font-face rule(s) were ignored; furniture may reuse a face acquired by the body but cannot acquire one"
    )]
    FurnitureFontFacesIgnored { count: usize },

    #[error("authored font acquisition reached its {kind} ceiling of {maximum}")]
    AuthoredFontResourceLimit { kind: &'static str, maximum: usize },

    #[error(
        "{count} authored-font text run(s) were kept in visible fallback because {reason}; first: {first:?}"
    )]
    AuthoredTextRefused {
        count: usize,
        reason: &'static str,
        first: String,
    },

    #[error(
        "{count} oversized authored-font token(s) were shaped in grapheme-safe chunks; cross-chunk shaping context was unavailable; first: {first:?}"
    )]
    AuthoredShapingContextSplit { count: usize, first: String },

    #[error(
        "{count} used authored font face(s) could not be embedded within the subset-output contract; their text kept visible fallback"
    )]
    AuthoredFontsNotEmbedded { count: usize },

    #[error(
        "{count} box(es) are painted past the {edge} edge of the paper, the \
         furthest by {worst}pt: {remedy}"
    )]
    BoxOutsideTheSheet {
        count: usize,
        worst: f32,
        edge: SheetEdge,
        remedy: &'static str,
    },

    #[error(
        "{count} line(s) reach past the edge of the paper, the furthest by \
         {worst}pt: this is the text's ADVANCE, so some of it may be side \
         bearing rather than ink — reduce the font size, allow a break, or \
         widen the page"
    )]
    TextReachesPastTheSheet { count: usize, worst: f32 },

    #[error(
        "{count} image(s) were scaled down to fit, the smallest to {smallest} \
         of the size asked for: they are indivisible, so fitting them keeps \
         the whole picture where overflowing would lose part of it"
    )]
    ImageScaledToFit { count: usize, smallest: f32 },

    #[error(
        "the {band} band's HTML needs {needed}pt of {axis} but its margin box \
         has only {available}pt, on {count} page(s) starting at page \
         {first_page}: {remedy}"
    )]
    FurnitureTooBig {
        band: &'static str,

        count: usize,
        first_page: usize,

        remedy: &'static str,

        axis: super::Axis,
        needed: f32,
        available: f32,
    },

    #[error(
        "the {band} band's HTML declares {kind} ({count}), which this version \
         does not draw in a margin: the text was drawn and the rest was not"
    )]
    FurnitureInkNotDrawn {
        band: &'static str,
        kind: &'static str,
        count: usize,
    },

    #[error(
        "{count} image(s) in the band are not drawn: this version draws text \
         in a margin band and nothing else. The alt text was rendered instead"
    )]
    FurnitureImageNotDrawn { count: usize },

    #[error(
        "the {box_name} box of the {band} band was declared in the document \
         and is drawn by the band's HTML instead: a subdocument occupies one \
         box, and the other two are unaffected"
    )]
    FurnitureBoxDisplaced {
        band: &'static str,
        box_name: &'static str,
    },

    #[error("the {band} band's HTML could not be compiled and was not drawn: {reason}")]
    FurnitureDocumentRefused { band: &'static str, reason: String },

    #[error("the {band} band's HTML uses {notice:?}")]
    FurnitureTemplate {
        band: &'static str,
        notice: super::TemplateNotice,
    },

    #[error("in the {band} band's HTML: {inner}")]
    FurnitureDocument {
        band: &'static str,
        inner: Box<Warning>,
    },

    #[error(
        "{count} inline element(s) declare a border. Borders on inline boxes \
         are not implemented and were not drawn"
    )]
    BorderOnInlineBox { count: usize },

    #[error(
        "{count} bordered box(es) have content on one page but a border box \
         that leaves it; their borders were not drawn. Shrinking a border box \
         to its page is not implemented"
    )]
    BorderLeavesItsPage { count: usize },

    #[error(
        "{count} border(s) enclose content that does not sit inside them — \
         usually a negative margin pulling content out of its own box — and \
         were not drawn"
    )]
    BorderDoesNotEncloseItsContent { count: usize },

    #[error(
        "{count} table row(s) or row group(s) declare a border. Row and \
         row-group boxes are not generated, so their borders were not drawn"
    )]
    BorderOnRowOrRowGroup { count: usize },

    #[error(
        "{count} image(s) share a line with other content and were not drawn; \
         their alt text was rendered instead. This version draws an image that \
         is the only content on its line"
    )]
    ImageSharesItsLine { count: usize },

    #[error(
        "{edge} asks for {needed}pt of a margin that leaves {available}pt; that band was not drawn"
    )]
    FurnitureRefused {
        edge: &'static str,
        needed: f32,
        available: f32,
    },

    #[error(
        "page furniture slots overlap on {count} page(s); all of them are \
         drawn where they were placed"
    )]
    FurnitureSlotsOverlap { count: usize },

    #[error("{name} was asked for and no such fact was supplied; it is empty")]
    FurnitureFactMissing { name: &'static str },

    #[error("{count} image(s) were not drawn: {cause}")]
    ImageSkipped { count: usize, cause: &'static str },

    #[error(
        "{count} image(s) carry a colour profile this version ignores; their colours may shift"
    )]
    ImageColourProfileIgnored { count: usize },

    #[error(
        "{count} line(s) taller than the page had empty leading trimmed to \
         fit ({points}pt in total). No content was lost: the text they carry \
         is drawn, and only space with nothing in it was removed"
    )]
    LeadingTrimmedToFit { count: usize, points: f32 },

    #[error(
        "{count} bordered block(s) named no page and their borders were not \
         drawn. This is not a limitation: please report the document"
    )]
    BorderOnFragmentedBlock { count: usize },

    #[error(
        "{count} HTML attribute(s) that affect layout are not implemented \
         (for example: {}). They were ignored.",
        examples.join(", ")
    )]
    HtmlAttributesNotImplemented { count: usize, examples: Vec<String> },

    #[error(
        "{count} character(s) could not be represented in the document's \
         encoding and were replaced with '?' (first was {first:?}); \
         embedded fonts will lift this limit"
    )]
    UnrepresentableCharacters { count: usize, first: char },

    #[error("could not read {path}: {cause} — continued without it")]
    ResourceSkipped { path: String, cause: String },

    #[error(
        "links were not created (fragment: {fragment}, relative: {relative}, \
         scheme-relative: {scheme_relative}, disallowed scheme: \
         {disallowed_scheme}, malformed: {malformed}, oversized: {oversized}, \
         unavailable in profile: {profile_unavailable}, unavailable in \
         furniture: {furniture_unavailable})"
    )]
    LinksNotCreated {
        fragment: usize,
        relative: usize,
        scheme_relative: usize,
        disallowed_scheme: usize,
        malformed: usize,
        oversized: usize,
        profile_unavailable: usize,
        furniture_unavailable: usize,
    },

    #[error(
        "{count} hyperlink annotation(s) were omitted after the document's \
         {limit}-annotation safety limit was reached"
    )]
    LinkAnnotationsOmitted { count: usize, limit: usize },

    #[error(
        "recovered from {count} markup error(s); the first was: {first}. \
         The document was rendered as a browser would interpret it."
    )]
    MarkupRecovered { count: usize, first: String },

    #[error(
        "{count} CSS declaration(s) use properties this version does not \
         implement (for example: {}). They were ignored.",
        examples.join(", ")
    )]
    CssNotImplemented { count: usize, examples: Vec<String> },

    #[error(
        "{count} CSS declaration(s) were applied as something else (for \
         example: {}). The page differs from what was written.",
        examples.join(", ")
    )]
    CssTranslated { count: usize, examples: Vec<String> },

    #[error("{count} malformed CSS declaration(s) could not be parsed and were skipped")]
    CssMalformed { count: usize },

    #[error(
        "a malformed CSS block was never closed; it and everything after it in that \
         stylesheet were skipped"
    )]
    CssUnterminatedBlock,

    #[error(
        "{count} media quer(y/ies) use conditions this version cannot \
         evaluate (for example: {}); their rules were not applied.",
        examples.join(", ")
    )]
    MediaQueriesNotSupported { count: usize, examples: Vec<String> },

    #[error(
        "{count} media quer(y/ies) could not be parsed (for example: {}); \
         their rules were not applied.",
        examples.join(", ")
    )]
    MediaQueriesMalformed { count: usize, examples: Vec<String> },

    #[error(
        "{count} conditional group(s) nest deeper than this version parses; \
         they were skipped whole"
    )]
    MediaNestingTooDeep { count: usize },

    #[error(
        "{count} CSS declaration(s) parsed but resolved to a value this \
         version cannot represent (for example: {}). They were ignored, and \
         the value that would otherwise apply was kept.",
        examples.join(", ")
    )]
    CssValueOutOfRange { count: usize, examples: Vec<String> },

    #[error(
        "could not load stylesheet {path}: {cause} — the document was \
         rendered without it"
    )]
    StylesheetSkipped { path: String, cause: String },

    #[error("stylesheet {path}: {}", issues.join("; "))]
    StylesheetDiagnostics { path: String, issues: Vec<String> },

    #[error(
        "stylesheet {href} was not loaded: this document has no base to \
         resolve relative references against (it did not come from a file)"
    )]
    StylesheetUnresolvable { href: String },

    #[error(
        "{count} selector(s) use syntax this version does not support (for \
         example: {}) and their rules were not applied.",
        examples.join(", ")
    )]
    SelectorsNotSupported { count: usize, examples: Vec<String> },

    #[error(
        "{count} page selector(s) are outside this version's unqualified @page support (for example: {}); their page rules were not applied.",
        examples.join(", ")
    )]
    CssPageSelectorNotSupported { count: usize, examples: Vec<String> },

    #[error("authored @page size or margins were ignored: {reason}")]
    AuthorPageIgnored { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RenderOutcome {
    pub pdf: PdfBytes,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SheetEdge {
    Left,

    Right,
}

impl std::fmt::Display for SheetEdge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}
