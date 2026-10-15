use super::{ComputedStyle, FlexContainerUpdate, FlexItemUpdate, FontSize, Property, Sampled, css};

pub(super) fn apply(
    property: Property,
    style: &mut ComputedStyle,
    parent_font_size: FontSize,
    borders: &mut SpecifiedEdges,
    paint: &mut SpecifiedPaint,
    hidden: &mut bool,
    out_of_range: &mut Sampled,
) {
    match property {
        Property::FontSize(spacing) => {
            match spacing
                .resolve(parent_font_size)
                .and_then(|length| FontSize::try_new(length.get()))
            {
                Ok(resolved) => style.font_size = resolved,

                Err(_) => out_of_range.record("font-size"),
            }
        }

        Property::Color(specified) => paint.foreground = Some(specified),

        Property::Background(value) => paint.background = Some(value),
        Property::Margin(edges) => style.margin = edges,
        Property::Padding(edges) => style.padding = edges,
        Property::LineHeight(spacing) => style.line_height = spacing,
        Property::Width(width) => style.width = width,
        Property::BoxSizing(value) => style.box_sizing = value,
        Property::TextAlign(alignment) => style.text_align = alignment,
        Property::FontFamily { names, fallback } => {
            style.font_families = names;
            style.font_family = fallback;
        }
        Property::FontWeight(weight) => style.font_weight = weight,
        Property::FontStyle(font_style) => style.font_style = font_style,
        Property::Border(update) => borders.absorb(&update),

        Property::BorderCollapseSeparate => {}
        Property::DisplayNone => *hidden = true,

        Property::Display(display) => {
            *hidden = false;
            style.display = display;
        }
        Property::FlexContainer(update) => match update {
            FlexContainerUpdate::Nowrap => {}
            FlexContainerUpdate::Direction(value) => style.flex_container.direction = value,
            FlexContainerUpdate::Justify(value) => style.flex_container.justify = value,
            FlexContainerUpdate::AlignItems(value) => style.flex_container.align_items = value,
            FlexContainerUpdate::RowGap(value) => style.flex_container.row_gap = value,
            FlexContainerUpdate::ColumnGap(value) => style.flex_container.column_gap = value,
            FlexContainerUpdate::Gap(row, column) => {
                style.flex_container.row_gap = row;
                style.flex_container.column_gap = column;
            }
        },
        Property::FlexItem(update) => match update {
            FlexItemUpdate::Grow(value) => style.flex_item.grow = value,
            FlexItemUpdate::Shrink(value) => style.flex_item.shrink = value,
            FlexItemUpdate::Basis(value) => style.flex_item.basis = value,
            FlexItemUpdate::AlignSelf(value) => style.flex_item.align_self = value,
            FlexItemUpdate::Flex {
                grow,
                shrink,
                basis,
            } => {
                style.flex_item.grow = grow;
                style.flex_item.shrink = shrink;
                style.flex_item.basis = basis;
            }
        },
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Specified {
    pub(super) width: crate::contracts::Spacing,
    pub(super) style: crate::contracts::BorderStyle,
    pub(super) colour: css::SpecifiedColour,
}

impl Default for Specified {
    fn default() -> Self {
        Self {
            width: crate::contracts::Spacing::points(3.0),
            style: crate::contracts::BorderStyle::None,
            colour: css::SpecifiedColour::CurrentColor,
        }
    }
}

impl Specified {
    pub(super) fn absorb(&mut self, components: css::BorderComponents) {
        if let Some(width) = components.width {
            self.width = width;
        }
        if let Some(style) = components.style {
            self.style = style;
        }
        if let Some(colour) = components.colour {
            self.colour = colour;
        }
    }

    pub(super) fn promote(
        self,
        font_size: crate::contracts::FontSize,
        current: crate::contracts::Rgb,
    ) -> crate::contracts::BorderSide {
        match self.style {
            crate::contracts::BorderStyle::None => crate::contracts::BorderSide::None,
            crate::contracts::BorderStyle::Solid => match self.width.resolve(font_size) {
                Ok(width) => {
                    crate::contracts::BorderSide::solid(width, self.colour.resolve(current))
                }

                Err(_) => crate::contracts::BorderSide::None,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct SpecifiedPaint {
    pub(super) foreground: Option<css::SpecifiedColour>,
    pub(super) background: Option<css::BackgroundValue>,
}

pub(super) struct Paint {
    pub(super) colour: crate::contracts::Rgb,
    pub(super) surface: crate::contracts::Surface,
    pub(super) paints: bool,

    pub(super) refusal: Option<String>,
}

impl SpecifiedPaint {
    pub(super) fn promote(
        self,
        inherited: crate::contracts::Surface,
        inherited_colour: crate::contracts::Rgb,
        inherited_paints: bool,
        display: crate::contracts::Display,
        colours: crate::contracts::Colours,
    ) -> Paint {
        if colours == crate::contracts::Colours::Off {
            return Paint {
                colour: inherited_colour,
                surface: crate::contracts::Surface::Canvas,

                paints: inherited_paints || self.background.is_some(),
                refusal: None,
            };
        }

        let foreground = self.foreground.map_or(inherited_colour, |specified| {
            specified.resolve(inherited_colour)
        });

        let shared_host = match display {
            crate::contracts::Display::Block
            | crate::contracts::Display::Flex
            | crate::contracts::Display::InlineFlex
            | crate::contracts::Display::Table
            | crate::contracts::Display::RowGroup
            | crate::contracts::Display::Row => None,
            crate::contracts::Display::Inline => Some("an inline box"),
            crate::contracts::Display::Cell => Some("a table cell"),
        };

        let refusal = match &self.background {
            None | Some(css::BackgroundValue::Transparent) => None,
            Some(css::BackgroundValue::Unhonourable(what)) => Some(what.clone()),
            Some(css::BackgroundValue::Colour(_)) => {
                shared_host.map(|host| format!("background-color on {host}"))
            }
        };
        match refusal {
            Some(what) => Paint {
                colour: inherited_colour,
                surface: inherited,
                paints: inherited_paints,
                refusal: Some(what),
            },
            None => Paint {
                colour: foreground,
                surface: match self.background {
                    None
                    | Some(
                        css::BackgroundValue::Transparent | css::BackgroundValue::Unhonourable(_),
                    ) => inherited,
                    Some(css::BackgroundValue::Colour(specified)) => {
                        crate::contracts::Surface::Colour(specified.resolve(foreground))
                    }
                },
                paints: inherited_paints || self.background.is_some(),
                refusal: None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SpecifiedEdges {
    pub(super) top: Specified,
    pub(super) right: Specified,
    pub(super) bottom: Specified,
    pub(super) left: Specified,
}

impl SpecifiedEdges {
    pub(super) fn absorb(&mut self, update: &css::BorderUpdate) {
        use css::BorderSideName as Side;
        for (side, specified) in [
            (Side::Top, &mut self.top),
            (Side::Right, &mut self.right),
            (Side::Bottom, &mut self.bottom),
            (Side::Left, &mut self.left),
        ] {
            specified.absorb(update.side(side));
        }
    }

    pub(super) fn promote(
        self,
        font_size: crate::contracts::FontSize,
        current: crate::contracts::Rgb,
    ) -> crate::contracts::BorderEdges {
        crate::contracts::BorderEdges {
            top: self.top.promote(font_size, current),
            right: self.right.promote(font_size, current),
            bottom: self.bottom.promote(font_size, current),
            left: self.left.promote(font_size, current),
        }
    }
}
