use super::{Error, FontSize, Length};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    red: f32,
    green: f32,
    blue: f32,
}

impl Rgb {
    pub const BLACK: Self = Self {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
    };

    pub fn try_new(red: f32, green: f32, blue: f32) -> Result<Self, Error> {
        for channel in [red, green, blue] {
            if !channel.is_finite() || !(0.0..=1.0).contains(&channel) {
                return Err(Error::ColourOutOfRange { channel });
            }
        }
        Ok(Self { red, green, blue })
    }

    #[must_use]
    pub fn from_bytes(red: u8, green: u8, blue: u8) -> Self {
        let scale = |channel: u8| f32::from(channel) / 255.0;
        Self {
            red: scale(red),
            green: scale(green),
            blue: scale(blue),
        }
    }

    #[must_use]
    pub const fn channels(self) -> (f32, f32, f32) {
        (self.red, self.green, self.blue)
    }

    #[must_use]
    pub fn is_black(self) -> bool {
        self == Self::BLACK
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BorderSide {
    #[default]
    None,
    Solid {
        width: Length,
        colour: Rgb,
    },
}

impl BorderSide {
    #[must_use]
    pub fn solid(width: Length, colour: Rgb) -> Self {
        if width.get() > 0.0 {
            Self::Solid { width, colour }
        } else {
            Self::None
        }
    }

    #[must_use]
    pub fn used_width(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Solid { width, .. } => width.get(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BorderEdges {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

impl BorderEdges {
    #[must_use]
    pub fn horizontal(self) -> f32 {
        self.left.used_width() + self.right.used_width()
    }

    #[must_use]
    pub fn vertical(self) -> f32 {
        self.top.used_width() + self.bottom.used_width()
    }

    #[must_use]
    pub fn any(self) -> bool {
        self.top != BorderSide::None
            || self.right != BorderSide::None
            || self.bottom != BorderSide::None
            || self.left != BorderSide::None
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ComputedStyle {
    pub font_size: FontSize,
    pub margin: SpacingEdges,
    pub padding: SpacingEdges,

    pub line_height: Spacing,
    pub width: Width,
    pub(crate) box_sizing: BoxSizing,
    pub display: Display,

    pub(crate) flex_container: FlexContainerStyle,

    pub(crate) flex_item: FlexItemStyle,
    pub text_align: TextAlign,
    pub font_family: FontFamily,

    pub(crate) font_families: std::sync::Arc<[String]>,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,

    pub border: BorderEdges,

    pub surface: Surface,

    pub paints: bool,

    pub colour: Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Surface {
    #[default]
    Canvas,
    Colour(Rgb),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BorderStyle {
    #[default]
    None,
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    Block,
    #[default]
    Inline,

    Flex,

    InlineFlex,

    Table,

    RowGroup,
    Row,
    Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FlexDirection {
    #[default]
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FlexJustify {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FlexAlign {
    #[default]
    Stretch,
    Start,
    End,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum FlexBasis {
    #[default]
    Auto,
    Content,
    Fixed(Spacing),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlexFactor(f32);

impl FlexFactor {
    pub(crate) const ZERO: Self = Self(0.0);
    pub(crate) const ONE: Self = Self(1.0);

    pub(crate) fn new(value: f32) -> Option<Self> {
        (value.is_finite() && value >= 0.0).then_some(Self(value))
    }

    pub(crate) fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlexContainerStyle {
    pub(crate) direction: FlexDirection,
    pub(crate) justify: FlexJustify,
    pub(crate) align_items: FlexAlign,
    pub(crate) row_gap: Spacing,
    pub(crate) column_gap: Spacing,
}

impl Default for FlexContainerStyle {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            justify: FlexJustify::Start,
            align_items: FlexAlign::Stretch,
            row_gap: Spacing::points(0.0),
            column_gap: Spacing::points(0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlexItemStyle {
    pub(crate) grow: FlexFactor,
    pub(crate) shrink: FlexFactor,
    pub(crate) basis: FlexBasis,
    pub(crate) align_self: Option<FlexAlign>,
}

impl Default for FlexItemStyle {
    fn default() -> Self {
        Self {
            grow: FlexFactor::ZERO,
            shrink: FlexFactor::ONE,
            basis: FlexBasis::Auto,
            align_self: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontFamily {
    #[default]
    Times,
    Helvetica,
    Courier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Edges {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

impl Edges {
    #[must_use]
    pub const fn uniform(value: Length) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    #[must_use]
    pub const fn new(top: Length, right: Length, bottom: Length, left: Length) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self::uniform(Length::ZERO)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Spacing {
    pub points: f32,
    pub em: f32,
}

impl Spacing {
    pub const ZERO: Self = Self {
        points: 0.0,
        em: 0.0,
    };

    #[must_use]
    pub const fn points(value: f32) -> Self {
        Self {
            points: value,
            em: 0.0,
        }
    }

    #[must_use]
    pub const fn em(value: f32) -> Self {
        Self {
            points: 0.0,
            em: value,
        }
    }

    pub fn resolve(self, font_size: FontSize) -> Result<Length, Error> {
        Length::try_new(
            self.em.mul_add(font_size.get(), self.points),
            "resolving a relative length",
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Width {
    #[default]
    Auto,

    Fixed(Spacing),

    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct SpacingEdges {
    pub top: Spacing,
    pub right: Spacing,
    pub bottom: Spacing,
    pub left: Spacing,
}

impl SpacingEdges {
    #[must_use]
    pub const fn uniform(value: Spacing) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn resolve(self, font_size: FontSize) -> Result<Edges, Error> {
        Ok(Edges {
            top: self.top.resolve(font_size)?,
            right: self.right.resolve(font_size)?,
            bottom: self.bottom.resolve(font_size)?,
            left: self.left.resolve(font_size)?,
        })
    }
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct StyledNode {
    pub tag: Option<String>,

    pub text: Option<String>,
    pub style: ComputedStyle,

    pub image: Option<ImageRequest>,

    pub(crate) link: Option<super::LinkTarget>,
    pub children: Vec<StyledNode>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ImageRequest {
    pub source: String,

    pub alt: Option<String>,

    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct StyledTree {
    pub(crate) author_page: super::AuthorPage,
    pub(crate) font_faces: Vec<crate::font::FaceRequest>,

    pub furniture: super::Furniture,
    pub root: StyledNode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[non_exhaustive]
pub(crate) struct RootSeed {
    pub font_size: Option<FontSize>,
    pub font_family: Option<FontFamily>,
    pub colour: Option<Rgb>,
}
