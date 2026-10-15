use crate::contracts::{BorderStyle, Rgb, Spacing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sides {
    All,
    One(Side),
}

impl Sides {
    const fn value_cap(self) -> usize {
        match self {
            Self::All => 4,
            Self::One(_) => 1,
        }
    }

    fn only(side: &str) -> Option<Self> {
        match side {
            "top" => Some(Self::One(Side::Top)),
            "right" => Some(Self::One(Side::Right)),
            "bottom" => Some(Self::One(Side::Bottom)),
            "left" => Some(Self::One(Side::Left)),
            _ => None,
        }
    }

    pub(crate) fn covers(self, side: Side) -> bool {
        match self {
            Self::All => true,
            Self::One(named) => named == side,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Components {
    pub(crate) width: Option<Spacing>,
    pub(crate) style: Option<BorderStyle>,
    pub(crate) colour: Option<SpecifiedColour>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Update {
    pub(crate) top: Components,
    pub(crate) right: Components,
    pub(crate) bottom: Components,
    pub(crate) left: Components,
}

impl Update {
    fn uniform(sides: Sides, components: Components) -> Self {
        let pick = |side: Side| {
            if sides.covers(side) {
                components
            } else {
                Components::default()
            }
        };
        Self {
            top: pick(Side::Top),
            right: pick(Side::Right),
            bottom: pick(Side::Bottom),
            left: pick(Side::Left),
        }
    }

    pub(crate) fn side(self, side: Side) -> Components {
        match side {
            Side::Top => self.top,
            Side::Right => self.right,
            Side::Bottom => self.bottom,
            Side::Left => self.left,
        }
    }
}

fn expand<T: Copy>(values: &[T]) -> Option<[T; 4]> {
    match values {
        [all] => Some([*all, *all, *all, *all]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

pub(crate) enum Parsed {
    Update(Update),

    Refused(String),

    Translated { update: Update, what: String },
    Malformed,
}

pub(crate) fn addresses(name: &str) -> Option<(Sides, Component)> {
    let rest = name.strip_prefix("border")?;
    if rest.is_empty() {
        return Some((Sides::All, Component::Shorthand));
    }

    let rest = rest.strip_prefix('-')?;
    if let Some(component) = Component::of(rest) {
        return Some((Sides::All, component));
    }
    let (side, tail) = match rest.find('-') {
        Some(index) => rest.split_at(index),
        None => (rest, ""),
    };
    let sides = Sides::only(side)?;
    if tail.is_empty() {
        return Some((sides, Component::Shorthand));
    }
    Some((sides, Component::of(tail.trim_start_matches('-'))?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Component {
    Shorthand,
    Width,
    Style,
    Colour,
}

impl Component {
    fn of(name: &str) -> Option<Self> {
        match name {
            "width" => Some(Self::Width),
            "style" => Some(Self::Style),
            "color" | "colour" => Some(Self::Colour),
            _ => None,
        }
    }
}

pub(crate) fn css_wide_keyword(parser: &mut cssparser::Parser<'_, '_>) -> Option<String> {
    let before = parser.state();
    let found = parser
        .expect_ident()
        .ok()
        .map(|ident| ident.to_ascii_lowercase())
        .filter(|name| {
            matches!(
                name.as_str(),
                "inherit" | "initial" | "unset" | "revert" | "revert-layer"
            )
        })
        .filter(|_| parser.is_exhausted());
    if found.is_none() {
        parser.reset(&before);
    }
    found
}

pub(crate) fn parse(
    sides: Sides,
    component: Component,
    parser: &mut cssparser::Parser<'_, '_>,
) -> Parsed {
    if let Some(keyword) = css_wide_keyword(parser) {
        return Parsed::Refused(format!("border: {keyword}"));
    }
    match component {
        Component::Shorthand => shorthand(sides, parser),
        Component::Width => longhand(sides, parser, width, |value| Components {
            width: Some(value),
            ..Components::default()
        }),
        Component::Style => styles(sides, parser),
        Component::Colour => colours(sides, parser),
    }
}

fn longhand<T: Copy>(
    sides: Sides,
    parser: &mut cssparser::Parser<'_, '_>,
    mut read: impl FnMut(&mut cssparser::Parser<'_, '_>) -> Option<T>,
    into: impl Fn(T) -> Components,
) -> Parsed {
    let mut values: Vec<T> = Vec::new();
    while values.len() < sides.value_cap() && !parser.is_exhausted() {
        if stop_at_important(parser) {
            break;
        }
        match parser.try_parse(|p| read(p).ok_or(())) {
            Ok(value) => values.push(value),
            Err(()) => return Parsed::Malformed,
        }
    }
    let Some(expanded) = expand(&values) else {
        return Parsed::Malformed;
    };
    Parsed::Update(per_side(sides, expanded.map(&into)))
}

fn per_side(sides: Sides, components: [Components; 4]) -> Update {
    let [top, right, bottom, left] = components;
    let pick = |side: Side, value: Components| {
        if sides.covers(side) {
            value
        } else {
            Components::default()
        }
    };
    Update {
        top: pick(Side::Top, top),
        right: pick(Side::Right, right),
        bottom: pick(Side::Bottom, bottom),
        left: pick(Side::Left, left),
    }
}

fn styles(sides: Sides, parser: &mut cssparser::Parser<'_, '_>) -> Parsed {
    let mut values: Vec<BorderStyle> = Vec::new();
    let mut translated: Option<String> = None;
    while values.len() < sides.value_cap() && !parser.is_exhausted() {
        if stop_at_important(parser) {
            break;
        }
        match style(parser) {
            Style::Used(used) => values.push(used),
            Style::AsSolid(what) => {
                values.push(BorderStyle::Solid);
                translated = Some(what);
            }
            Style::Refused(what) => return Parsed::Refused(what),
            Style::Malformed => return Parsed::Malformed,
        }
    }
    let Some(expanded) = expand(&values) else {
        return Parsed::Malformed;
    };
    let update = per_side(
        sides,
        expanded.map(|style| Components {
            style: Some(style),
            ..Components::default()
        }),
    );
    match translated {
        Some(what) => Parsed::Translated { update, what },
        None => Parsed::Update(update),
    }
}

fn colours(sides: Sides, parser: &mut cssparser::Parser<'_, '_>) -> Parsed {
    let mut values: Vec<SpecifiedColour> = Vec::new();
    while values.len() < sides.value_cap() && !parser.is_exhausted() {
        if stop_at_important(parser) {
            break;
        }
        match colour(parser, BORDER_NAMING) {
            Colour::Used(specified) => values.push(specified),
            Colour::Refused(what) => return Parsed::Refused(what),
            Colour::Malformed => return Parsed::Malformed,
        }
    }
    let Some(expanded) = expand(&values) else {
        return Parsed::Malformed;
    };
    Parsed::Update(per_side(
        sides,
        expanded.map(|specified| Components {
            colour: Some(specified),
            ..Components::default()
        }),
    ))
}

fn stop_at_important(parser: &mut cssparser::Parser<'_, '_>) -> bool {
    let before = parser.state();
    let bang = parser.expect_delim('!').is_ok();
    parser.reset(&before);
    bang
}

fn shorthand(sides: Sides, parser: &mut cssparser::Parser<'_, '_>) -> Parsed {
    let mut written = Components {
        width: Some(MEDIUM),
        style: Some(BorderStyle::None),
        colour: Some(SpecifiedColour::CurrentColor),
    };
    let mut translated: Option<String> = None;
    let (mut saw_width, mut saw_style, mut saw_colour) = (false, false, false);
    let mut components = 0_usize;

    while !parser.is_exhausted() {
        if stop_at_important(parser) {
            break;
        }
        if !saw_width && let Ok(points) = parser.try_parse(|p| width(p).ok_or(())) {
            written.width = Some(points);
            saw_width = true;
            components = components.saturating_add(1);
            continue;
        }
        if !saw_style {
            match parser.try_parse(|p| match style(p) {
                Style::Malformed => Err(()),
                Style::Used(used) => Ok(Style::Used(used)),
                Style::AsSolid(what) => Ok(Style::AsSolid(what)),
                Style::Refused(what) => Ok(Style::Refused(what)),
            }) {
                Ok(Style::Used(used)) => {
                    written.style = Some(used);
                    saw_style = true;
                    components = components.saturating_add(1);
                    continue;
                }
                Ok(Style::AsSolid(what)) => {
                    written.style = Some(BorderStyle::Solid);
                    translated = Some(what);
                    saw_style = true;
                    components = components.saturating_add(1);
                    continue;
                }

                Ok(Style::Refused(what)) => return Parsed::Refused(what),

                Ok(Style::Malformed) | Err(()) => {}
            }
        }
        if !saw_colour {
            match parser.try_parse(|p| match colour(p, BORDER_NAMING) {
                Colour::Malformed => Err(()),
                Colour::Used(specified) => Ok(Colour::Used(specified)),
                Colour::Refused(what) => Ok(Colour::Refused(what)),
            }) {
                Ok(Colour::Used(specified)) => {
                    written.colour = Some(specified);
                    saw_colour = true;
                    components = components.saturating_add(1);
                    continue;
                }
                Ok(Colour::Refused(what)) => return Parsed::Refused(what),
                Ok(Colour::Malformed) | Err(()) => {}
            }
        }
        return Parsed::Malformed;
    }

    if components == 0 {
        return Parsed::Malformed;
    }
    let update = Update::uniform(sides, written);
    match translated {
        Some(what) => Parsed::Translated { update, what },
        None => Parsed::Update(update),
    }
}

const MEDIUM: Spacing = Spacing::points(3.0);

fn width(parser: &mut cssparser::Parser<'_, '_>) -> Option<Spacing> {
    if let Ok(keyword) = parser.try_parse(|p| p.expect_ident().map(|i| i.to_ascii_lowercase())) {
        return match keyword.as_str() {
            "thin" => Some(Spacing::points(1.0)),
            "medium" => Some(MEDIUM),
            "thick" => Some(Spacing::points(5.0)),
            _ => None,
        };
    }

    let spacing = parser
        .try_parse(super::declarations::parse_length_value)
        .ok()?;
    (spacing.points >= 0.0 && spacing.em >= 0.0).then_some(spacing)
}

enum Style {
    Used(BorderStyle),

    AsSolid(String),
    Refused(String),
    Malformed,
}

fn style(parser: &mut cssparser::Parser<'_, '_>) -> Style {
    let Ok(keyword) = parser.try_parse(|p| p.expect_ident().map(|i| i.to_ascii_lowercase())) else {
        return Style::Malformed;
    };
    match keyword.as_str() {
        "solid" => Style::Used(BorderStyle::Solid),

        "none" | "hidden" => Style::Used(BorderStyle::None),

        "dashed" | "dotted" => Style::AsSolid(format!("border-style: {keyword} drawn as solid")),

        "double" | "groove" | "ridge" | "inset" | "outset" => {
            Style::Refused(format!("border-style: {keyword}"))
        }
        _ => Style::Malformed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SpecifiedColour {
    Explicit(Rgb),
    CurrentColor,
}

impl SpecifiedColour {
    pub(crate) fn resolve(self, current: Rgb) -> Rgb {
        match self {
            Self::Explicit(rgb) => rgb,
            Self::CurrentColor => current,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Naming {
    pub(super) value: &'static str,
    pub(super) keyword: &'static str,
}

pub(super) const BORDER_NAMING: Naming = Naming {
    value: "border colour",
    keyword: "border",
};

pub(super) enum Colour {
    Used(SpecifiedColour),
    Refused(String),
    Malformed,
}

pub(super) fn colour(parser: &mut cssparser::Parser<'_, '_>, naming: Naming) -> Colour {
    let Ok(token) = parser.next().cloned() else {
        return Colour::Malformed;
    };

    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "foreign token enum; every non-matching token is equally not a colour"
    )]
    match token {
        cssparser::Token::IDHash(hex) | cssparser::Token::Hash(hex) => match from_hex(&hex) {
            Some(rgb) => Colour::Used(SpecifiedColour::Explicit(rgb)),

            None if matches!(hex.len(), 4 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
                Colour::Refused(format!("{}: #{hex} (alpha)", naming.value))
            }
            None => Colour::Malformed,
        },

        cssparser::Token::Ident(name) if name.eq_ignore_ascii_case("transparent") => {
            Colour::Refused(format!("{}: transparent", naming.value))
        }

        cssparser::Token::Ident(name)
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "inherit" | "initial" | "unset" | "revert" | "revert-layer"
            ) =>
        {
            Colour::Refused(format!("{}: {name}", naming.keyword))
        }
        cssparser::Token::Ident(name) => match named(&name.to_ascii_lowercase()) {
            Some(rgb) => Colour::Used(SpecifiedColour::Explicit(rgb)),

            None if name.eq_ignore_ascii_case("currentcolor") => {
                Colour::Used(SpecifiedColour::CurrentColor)
            }
            None if is_colour_name(&name.to_ascii_lowercase()) => {
                Colour::Refused(format!("{}: {name}", naming.value))
            }
            None => Colour::Malformed,
        },
        cssparser::Token::Function(name) => Colour::Refused(format!("{}: {name}()", naming.value)),

        _ => Colour::Malformed,
    }
}

fn from_hex(hex: &str) -> Option<Rgb> {
    let channel = |slice: &str| u8::from_str_radix(slice, 16).ok();
    let (red, green, blue) = match hex.len() {
        3 => {
            let doubled: Vec<String> = hex.chars().map(|c| format!("{c}{c}")).collect();
            (
                channel(doubled.first()?)?,
                channel(doubled.get(1)?)?,
                channel(doubled.get(2)?)?,
            )
        }
        6 => (
            channel(hex.get(0..2)?)?,
            channel(hex.get(2..4)?)?,
            channel(hex.get(4..6)?)?,
        ),
        _ => return None,
    };

    Some(Rgb::from_bytes(red, green, blue))
}

fn named(name: &str) -> Option<Rgb> {
    let grey = |value: f32| Rgb::try_new(value, value, value).ok();
    match name {
        "black" => grey(0.0),
        "white" => grey(1.0),
        "gray" | "grey" => grey(128.0 / 255.0),
        "silver" => grey(192.0 / 255.0),
        "red" => Rgb::try_new(1.0, 0.0, 0.0).ok(),
        "lime" => Rgb::try_new(0.0, 1.0, 0.0).ok(),
        "blue" => Rgb::try_new(0.0, 0.0, 1.0).ok(),
        "green" => Rgb::try_new(0.0, 128.0 / 255.0, 0.0).ok(),
        "navy" => Rgb::try_new(0.0, 0.0, 128.0 / 255.0).ok(),
        "maroon" => Rgb::try_new(128.0 / 255.0, 0.0, 0.0).ok(),
        "yellow" => Rgb::try_new(1.0, 1.0, 0.0).ok(),
        _ => None,
    }
}

fn is_colour_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphabetic())
}
