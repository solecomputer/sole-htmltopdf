use super::{Sampled, Warning, css};

#[derive(Default)]
pub(super) struct Diagnostics {
    pub(super) unsupported: Sampled,
    pub(super) malformed: Sampled,

    pub(super) translated: Sampled,
    pub(super) media_unsupported: Sampled,
    pub(super) media_malformed: Sampled,
    pub(super) media_too_deep: usize,
    pub(super) unterminated: bool,
    pub(super) refused: Sampled,
    pub(super) page_selectors: Sampled,

    pub(super) out_of_range: Sampled,

    pub(super) html_attributes: Sampled,
}

pub(super) const UNIMPLEMENTED_ATTRIBUTES: &[&str] = &[
    "colspan",
    "rowspan",
    "cellpadding",
    "cellspacing",
    "bgcolor",
    "background",
    "align",
    "valign",
    "border",
    "nowrap",
    "width",
    "height",
];

impl Diagnostics {
    pub(super) fn absorb(&mut self, parsed: &css::Parsed) {
        self.unsupported.merge(&parsed.unsupported);
        self.malformed.merge(&parsed.malformed);
        self.translated.merge(&parsed.translated);
    }

    pub(super) fn into_warnings(self) -> Vec<Warning> {
        let mut warnings = Vec::new();
        if !self.unsupported.is_empty() {
            warnings.push(Warning::CssNotImplemented {
                count: self.unsupported.total,
                examples: self.unsupported.samples,
            });
        }
        if !self.translated.is_empty() {
            warnings.push(Warning::CssTranslated {
                count: self.translated.total,
                examples: self.translated.samples,
            });
        }
        if !self.malformed.is_empty() {
            warnings.push(Warning::CssMalformed {
                count: self.malformed.total,
            });
        }
        if self.unterminated {
            warnings.push(Warning::CssUnterminatedBlock);
        }
        if !self.media_unsupported.is_empty() {
            warnings.push(Warning::MediaQueriesNotSupported {
                count: self.media_unsupported.total,
                examples: self.media_unsupported.samples,
            });
        }
        if !self.media_malformed.is_empty() {
            warnings.push(Warning::MediaQueriesMalformed {
                count: self.media_malformed.total,
                examples: self.media_malformed.samples,
            });
        }
        if self.media_too_deep > 0 {
            warnings.push(Warning::MediaNestingTooDeep {
                count: self.media_too_deep,
            });
        }
        if !self.refused.is_empty() {
            warnings.push(Warning::SelectorsNotSupported {
                count: self.refused.total,
                examples: self.refused.samples,
            });
        }
        if !self.page_selectors.is_empty() {
            warnings.push(Warning::CssPageSelectorNotSupported {
                count: self.page_selectors.total,
                examples: self.page_selectors.samples,
            });
        }
        if !self.out_of_range.is_empty() {
            warnings.push(Warning::CssValueOutOfRange {
                count: self.out_of_range.total,
                examples: self.out_of_range.samples,
            });
        }
        if !self.html_attributes.is_empty() {
            warnings.push(Warning::HtmlAttributesNotImplemented {
                count: self.html_attributes.total,
                examples: self.html_attributes.samples,
            });
        }
        warnings
    }
}
