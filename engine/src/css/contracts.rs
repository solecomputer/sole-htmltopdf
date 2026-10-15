use super::border;
use super::border::SpecifiedColour;
use crate::contracts::{FontStyle, TextAlign};

pub(crate) const USER_AGENT_STYLESHEET: &str = include_str!("../../data/ua.css");

pub(crate) const MAX_LENGTH_POINTS: f32 = 1_000_000.0;

pub(crate) const NORMAL_LINE_HEIGHT: f32 = 1.2;

pub(super) const SAMPLE_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Property {
    FontSize(crate::contracts::Spacing),

    Margin(crate::contracts::SpacingEdges),

    Padding(crate::contracts::SpacingEdges),
    LineHeight(crate::contracts::Spacing),
    Width(crate::contracts::Width),
    BoxSizing(crate::contracts::BoxSizing),
    DisplayNone,
    Display(crate::contracts::Display),
    FlexContainer(FlexContainerUpdate),
    FlexItem(FlexItemUpdate),
    TextAlign(TextAlign),
    FontFamily {
        names: std::sync::Arc<[String]>,
        fallback: crate::contracts::FontFamily,
    },
    FontWeight(crate::contracts::FontWeight),
    FontStyle(FontStyle),

    Background(BackgroundValue),

    Color(SpecifiedColour),

    Border(border::Update),

    BorderCollapseSeparate,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FlexContainerUpdate {
    Nowrap,
    Direction(crate::contracts::FlexDirection),
    Justify(crate::contracts::FlexJustify),
    AlignItems(crate::contracts::FlexAlign),
    RowGap(crate::contracts::Spacing),
    ColumnGap(crate::contracts::Spacing),
    Gap(crate::contracts::Spacing, crate::contracts::Spacing),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FlexItemUpdate {
    Grow(crate::contracts::FlexFactor),
    Shrink(crate::contracts::FlexFactor),
    Basis(crate::contracts::FlexBasis),
    AlignSelf(Option<crate::contracts::FlexAlign>),
    Flex {
        grow: crate::contracts::FlexFactor,
        shrink: crate::contracts::FlexFactor,
        basis: crate::contracts::FlexBasis,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Declared {
    pub(crate) property: Property,
    pub(crate) important: bool,
}

pub(super) const COLOR_NAMING: border::Naming = border::Naming {
    value: "color",
    keyword: "color",
};

pub(super) const BACKGROUND_NAMING: border::Naming = border::Naming {
    value: "background-color",
    keyword: "background-color",
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BackgroundValue {
    Transparent,
    Colour(SpecifiedColour),

    Unhonourable(String),
}

#[derive(Debug, Default)]
pub(crate) struct Parsed {
    pub(crate) declarations: Vec<Declared>,

    pub(crate) unsupported: Sampled,

    pub(crate) malformed: Sampled,

    pub(crate) translated: Sampled,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Sampled {
    pub(crate) samples: Vec<String>,
    pub(crate) total: usize,
}

impl Sampled {
    pub(crate) fn record(&mut self, what: &str) {
        self.total = self.total.saturating_add(1);
        if self.samples.len() < SAMPLE_LIMIT && !self.samples.iter().any(|s| s == what) {
            self.samples.push(what.to_owned());
        }
    }

    pub(crate) fn merge(&mut self, other: &Self) {
        self.total = self.total.saturating_add(other.total);
        for sample in &other.samples {
            if self.samples.len() < SAMPLE_LIMIT && !self.samples.contains(sample) {
                self.samples.push(sample.clone());
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.total == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    UserAgent,

    Author,
}

pub(crate) struct Rule {
    pub(crate) origin: Origin,
    pub(crate) selectors: crate::selector::SelectorList,
    pub(crate) declarations: Vec<Declared>,
}

#[derive(Default)]
pub(crate) struct Sheet {
    pub(crate) rules: Vec<Rule>,
    pub(crate) font_faces: Vec<super::font_face::FaceRule>,
    pub(crate) unsupported: Sampled,
    pub(crate) malformed: Sampled,

    pub(crate) unterminated: bool,

    pub(crate) refused_selectors: Sampled,

    pub(crate) page_selectors: Sampled,

    pub(crate) page: super::page::PageRules,

    pub(crate) media_unsupported: Sampled,

    pub(crate) media_malformed: Sampled,

    pub(crate) media_too_deep: usize,
}
