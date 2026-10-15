use cssparser::{DeclarationParser, ParserInput, RuleBodyItemParser, RuleBodyParser};

use super::border;
use super::contracts::{
    BACKGROUND_NAMING, BackgroundValue, COLOR_NAMING, Declared, MAX_LENGTH_POINTS,
    NORMAL_LINE_HEIGHT, Parsed, Property,
};
use crate::contracts::{FontStyle, TextAlign};

pub(crate) fn parse_declarations(source: &str) -> Parsed {
    let mut collector = Collector::default();
    let malformed = drive_declarations(source, &mut collector);

    let mut parsed = collector.parsed;
    for _ in 0..malformed {
        parsed.malformed.record("<unparsable declaration>");
    }
    parsed
}

pub(super) fn drive_declarations<'i, P>(source: &'i str, collector: &mut P) -> usize
where
    P: DeclarationParser<'i, Declaration = (), Error = ()>
        + cssparser::AtRuleParser<'i, Prelude = (), AtRule = (), Error = ()>
        + cssparser::QualifiedRuleParser<'i, Prelude = (), QualifiedRule = (), Error = ()>
        + RuleBodyItemParser<'i, (), ()>,
{
    let mut input = ParserInput::new(source);
    let mut parser = cssparser::Parser::new(&mut input);
    let mut malformed = 0_usize;
    for result in RuleBodyParser::new(&mut parser, collector) {
        if result.is_err() {
            malformed = malformed.saturating_add(1);
        }
    }
    malformed
}

pub(super) fn finish_declaration<'i>(
    parser: &mut cssparser::Parser<'i, '_>,
) -> Result<bool, cssparser::ParseError<'i, ()>> {
    let important = take_important(parser);
    if parser.is_exhausted() {
        Ok(important)
    } else {
        Err(parser.new_custom_error::<(), ()>(()))
    }
}

#[derive(Default)]
struct Collector {
    parsed: Parsed,
}

impl<'i> DeclarationParser<'i> for Collector {
    type Declaration = ();
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        parser: &mut cssparser::Parser<'i, 't>,
        _declaration_start: &cssparser::ParserState,
    ) -> Result<(), cssparser::ParseError<'i, ()>> {
        let lowered = name.to_ascii_lowercase();
        match value_of(&lowered, parser) {
            Outcome::Value(property) => {
                let important = finish_declaration(parser)?;
                self.parsed.declarations.push(Declared {
                    property,
                    important,
                });
                Ok(())
            }

            Outcome::Refused(what) => {
                self.parsed.unsupported.record(&what);
                while parser.next().is_ok() {}
                Ok(())
            }

            Outcome::Unknown => {
                self.parsed.unsupported.record(&lowered);
                while parser.next().is_ok() {}
                Ok(())
            }

            Outcome::Translated { property, what } => {
                let important = finish_declaration(parser)?;
                self.parsed.translated.record(&what);
                self.parsed.declarations.push(Declared {
                    property,
                    important,
                });
                Ok(())
            }

            Outcome::Malformed => Err(parser.new_custom_error::<(), ()>(())),
        }
    }
}

pub(super) enum Outcome {
    Value(Property),

    Refused(String),

    Unknown,

    Malformed,

    Translated { property: Property, what: String },
}

#[allow(
    clippy::too_many_lines,
    reason = "the declaration dispatcher is exhaustive over the supported property vocabulary; splitting it would create a second routing table"
)]
fn value_of(name: &str, parser: &mut cssparser::Parser<'_, '_>) -> Outcome {
    if let Some((sides, component)) = border::addresses(name) {
        return match border::parse(sides, component, parser) {
            border::Parsed::Update(update) => Outcome::Value(Property::Border(update)),
            border::Parsed::Translated { update, what } => Outcome::Translated {
                property: Property::Border(update),
                what,
            },
            border::Parsed::Refused(what) => Outcome::Refused(what),
            border::Parsed::Malformed => Outcome::Malformed,
        };
    }
    if let Some(outcome) = super::flex::value_of(name, parser) {
        return outcome;
    }
    match name {
        "border-collapse" if border::css_wide_keyword(parser).is_some() => {
            Outcome::Refused("border-collapse: a CSS-wide keyword".to_owned())
        }
        "border-collapse" => match keyword(parser).as_deref() {
            Some("separate") => Outcome::Value(Property::BorderCollapseSeparate),

            Some("collapse") => Outcome::Refused("border-collapse: collapse".to_owned()),
            _ => Outcome::Malformed,
        },
        "font-size" => match parse_font_size(parser) {
            Ok(size) => Outcome::Value(Property::FontSize(size)),
            Err(()) => Outcome::Malformed,
        },
        "margin" => match parse_edges(parser) {
            Ok(edges) => Outcome::Value(Property::Margin(edges)),

            Err(EdgeError::Auto) => Outcome::Refused("margin: auto".to_owned()),
            Err(EdgeError::Malformed) => Outcome::Malformed,
        },
        "width" => match parse_width(parser) {
            Ok(width) => Outcome::Value(Property::Width(width)),
            Err(()) => Outcome::Malformed,
        },
        "box-sizing" => match keyword(parser).as_deref() {
            Some("content-box") => {
                Outcome::Value(Property::BoxSizing(crate::contracts::BoxSizing::ContentBox))
            }
            Some("border-box") => {
                Outcome::Value(Property::BoxSizing(crate::contracts::BoxSizing::BorderBox))
            }
            _ => Outcome::Malformed,
        },
        "line-height" => match parse_line_height(parser) {
            Ok(spacing) => Outcome::Value(Property::LineHeight(spacing)),
            Err(()) => Outcome::Malformed,
        },
        "padding" => match parse_edges(parser) {
            Ok(edges) => Outcome::Value(Property::Padding(edges)),

            Err(_) => Outcome::Malformed,
        },
        "display" => match keyword(parser).as_deref() {
            Some("none") => Outcome::Value(Property::DisplayNone),
            Some("block" | "flow-root" | "list-item") => {
                Outcome::Value(Property::Display(crate::contracts::Display::Block))
            }

            Some("inline" | "inline-block") => {
                Outcome::Value(Property::Display(crate::contracts::Display::Inline))
            }
            Some("flex") => Outcome::Value(Property::Display(crate::contracts::Display::Flex)),
            Some("inline-flex") => {
                Outcome::Value(Property::Display(crate::contracts::Display::InlineFlex))
            }
            Some("-webkit-box" | "-webkit-flex" | "-ms-flexbox") => {
                Outcome::Refused("display: legacy flex dialect".to_owned())
            }

            Some("table" | "inline-table") => {
                Outcome::Value(Property::Display(crate::contracts::Display::Table))
            }
            Some("table-row-group" | "table-header-group" | "table-footer-group") => {
                Outcome::Value(Property::Display(crate::contracts::Display::RowGroup))
            }
            Some("table-row") => Outcome::Value(Property::Display(crate::contracts::Display::Row)),
            Some("table-cell") => {
                Outcome::Value(Property::Display(crate::contracts::Display::Cell))
            }
            _ => Outcome::Malformed,
        },
        "text-align" => match keyword(parser).as_deref() {
            Some("left" | "start") => Outcome::Value(Property::TextAlign(TextAlign::Left)),
            Some("right" | "end") => Outcome::Value(Property::TextAlign(TextAlign::Right)),
            Some("center") => Outcome::Value(Property::TextAlign(TextAlign::Center)),

            Some("justify") => Outcome::Refused("text-align: justify".to_owned()),
            _ => Outcome::Malformed,
        },
        "font-family" => match parse_family_list(parser) {
            Err(()) => Outcome::Malformed,

            Ok(families) => match families.iter().find_map(|name| known_family(name)) {
                Some(fallback) => Outcome::Value(Property::FontFamily {
                    names: families.into(),
                    fallback,
                }),

                None if !families.is_empty() => Outcome::Value(Property::FontFamily {
                    names: families.into(),
                    fallback: crate::contracts::FontFamily::Times,
                }),
                None => Outcome::Malformed,
            },
        },
        "font-weight" => match parse_font_weight(parser) {
            Ok(weight) => Outcome::Value(Property::FontWeight(weight)),
            Err(()) => Outcome::Malformed,
        },
        "font-style" => match keyword(parser).as_deref() {
            Some("normal") => Outcome::Value(Property::FontStyle(FontStyle::Normal)),

            Some("italic" | "oblique") => Outcome::Value(Property::FontStyle(FontStyle::Italic)),
            _ => Outcome::Malformed,
        },
        "background" | "background-image" | "background-color" => background(name, parser),
        "color" => match border::colour(parser, COLOR_NAMING) {
            border::Colour::Used(specified) => Outcome::Value(Property::Color(specified)),
            border::Colour::Refused(what) => Outcome::Refused(what),
            border::Colour::Malformed => Outcome::Malformed,
        },
        _ => Outcome::Unknown,
    }
}

pub(super) fn keyword(parser: &mut cssparser::Parser<'_, '_>) -> Option<String> {
    parser.expect_ident().ok().map(|i| i.to_ascii_lowercase())
}

impl cssparser::AtRuleParser<'_> for Collector {
    type Prelude = ();
    type AtRule = ();
    type Error = ();
}

impl cssparser::QualifiedRuleParser<'_> for Collector {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = ();
}

impl RuleBodyItemParser<'_, (), ()> for Collector {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

pub(super) enum EdgeError {
    Auto,
    Malformed,
}

pub(super) fn parse_edges(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Result<crate::contracts::SpacingEdges, EdgeError> {
    use crate::contracts::SpacingEdges as Edges;

    let mut saw_auto = false;

    let mut values = Vec::new();
    while values.len() < 4 {
        if parser
            .try_parse(|p| p.expect_ident_matching("auto"))
            .is_ok()
        {
            saw_auto = true;
            continue;
        }
        let Ok(length) = parser.try_parse(parse_length_value) else {
            break;
        };
        values.push(length);
    }
    if saw_auto {
        return Err(EdgeError::Auto);
    }

    match values.as_slice() {
        [all] => Ok(Edges::uniform(*all)),
        [vertical, horizontal] => Ok(Edges {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        }),
        [top, horizontal, bottom] => Ok(Edges {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        }),
        [top, right, bottom, left] => Ok(Edges {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        }),
        _ => Err(EdgeError::Malformed),
    }
}

pub(super) fn parse_length_value(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Result<crate::contracts::Spacing, cssparser::ParseError<'static, ()>> {
    let token = parser.next().map_err(|_| cssparser::ParseError {
        kind: cssparser::ParseErrorKind::Custom(()),
        location: cssparser::SourceLocation { line: 0, column: 0 },
    })?;
    let fail = || cssparser::ParseError {
        kind: cssparser::ParseErrorKind::Custom(()),
        location: cssparser::SourceLocation { line: 0, column: 0 },
    };

    let spacing = if let cssparser::Token::Dimension { value, unit, .. } = token {
        match unit.to_ascii_lowercase().as_str() {
            "em" if value.abs() <= MAX_LENGTH_POINTS => {
                return Ok(crate::contracts::Spacing::em(*value));
            }
            "pt" => *value,
            "px" => *value * 72.0 / 96.0,
            "in" => *value * 72.0,
            "cm" => *value * 72.0 / 2.54,
            "mm" => *value * 72.0 / 25.4,
            _ => return Err(fail()),
        }
    } else if matches!(token, cssparser::Token::Number { value, .. } if *value == 0.0) {
        0.0
    } else {
        return Err(fail());
    };

    if spacing.is_finite() && spacing.abs() <= MAX_LENGTH_POINTS {
        Ok(crate::contracts::Spacing::points(spacing))
    } else {
        Err(fail())
    }
}

fn take_important(parser: &mut cssparser::Parser<'_, '_>) -> bool {
    parser
        .try_parse(|input| {
            input.expect_delim('!')?;
            input.expect_ident_matching("important")
        })
        .is_ok()
}

fn parse_font_size(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Result<crate::contracts::Spacing, ()> {
    use crate::contracts::Spacing;
    if let Ok(percentage) = parser.try_parse(cssparser::Parser::expect_percentage) {
        if percentage.abs() > MAX_LENGTH_POINTS {
            return Err(());
        }
        return Ok(Spacing::em(percentage));
    }
    let token = parser.next().map_err(|_| ())?.clone();
    let cssparser::Token::Dimension { value, unit, .. } = token else {
        return Err(());
    };
    let points = match unit.to_ascii_lowercase().as_str() {
        "pt" => value,

        "px" => value * 72.0 / 96.0,
        "in" => value * 72.0,
        "cm" => value * 72.0 / 2.54,
        "mm" => value * 72.0 / 25.4,

        "em" if value.abs() <= MAX_LENGTH_POINTS => return Ok(Spacing::em(value)),
        _ => return Err(()),
    };
    if points.is_finite() && points.abs() <= MAX_LENGTH_POINTS {
        Ok(Spacing::points(points))
    } else {
        Err(())
    }
}

fn parse_width(parser: &mut cssparser::Parser<'_, '_>) -> Result<crate::contracts::Width, ()> {
    use crate::contracts::Width;
    if parser
        .try_parse(|p| p.expect_ident_matching("auto"))
        .is_ok()
    {
        return Ok(Width::Auto);
    }
    if let Ok(percentage) = parser.try_parse(cssparser::Parser::expect_percentage) {
        if percentage < 0.0 {
            return Err(());
        }
        return Ok(Width::Percent(percentage));
    }
    let spacing = parser.try_parse(parse_length_value).map_err(|_| ())?;
    if spacing.points < 0.0 || spacing.em < 0.0 {
        return Err(());
    }
    Ok(Width::Fixed(spacing))
}

fn parse_line_height(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Result<crate::contracts::Spacing, ()> {
    use crate::contracts::Spacing;
    if parser
        .try_parse(|p| p.expect_ident_matching("normal"))
        .is_ok()
    {
        return Ok(Spacing::em(NORMAL_LINE_HEIGHT));
    }
    if let Ok(number) = parser.try_parse(cssparser::Parser::expect_number) {
        if number < 0.0 {
            return Err(());
        }
        return Ok(Spacing::em(number));
    }
    parser.try_parse(parse_length_value).map_err(|_| ())
}

fn parse_family_list(parser: &mut cssparser::Parser<'_, '_>) -> Result<Vec<String>, ()> {
    let mut families = Vec::new();
    loop {
        let mut name = String::new();
        loop {
            let start = parser.state();
            match parser.next() {
                Ok(cssparser::Token::Ident(ident)) => {
                    if !name.is_empty() {
                        name.push(' ');
                    }
                    name.push_str(&ident.to_ascii_lowercase());
                }
                Ok(cssparser::Token::QuotedString(text)) => {
                    if !name.is_empty() {
                        name.push(' ');
                    }
                    name.push_str(&text.to_ascii_lowercase());
                }

                _ => {
                    parser.reset(&start);
                    break;
                }
            }
        }
        if name.is_empty() {
            return Err(());
        }
        families.push(name);
        if parser.expect_comma().is_err() {
            break;
        }
    }
    Ok(families)
}

fn known_family(name: &str) -> Option<crate::contracts::FontFamily> {
    crate::metrics::family_named(name)
}

fn parse_font_weight(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Result<crate::contracts::FontWeight, ()> {
    use crate::contracts::FontWeight;
    let token = parser.next().map_err(|_| ())?.clone();

    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "foreign token enum; every non-matching token is equally not a weight"
    )]
    match token {
        cssparser::Token::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
            "normal" => Ok(FontWeight::Normal),
            "bold" => Ok(FontWeight::Bold),
            _ => Err(()),
        },
        cssparser::Token::Number { value, .. } => {
            if (1.0..=1000.0).contains(&value) {
                Ok(if value >= 600.0 {
                    FontWeight::Bold
                } else {
                    FontWeight::Normal
                })
            } else {
                Err(())
            }
        }
        _ => Err(()),
    }
}

fn unhonourable(parser: &mut cssparser::Parser<'_, '_>, what: &str) -> Outcome {
    while parser.next().is_ok() {}
    Outcome::Value(Property::Background(BackgroundValue::Unhonourable(
        what.to_owned(),
    )))
}

fn background(name: &str, parser: &mut cssparser::Parser<'_, '_>) -> Outcome {
    match name {
        "background" => match parser.try_parse(|p| match border::colour(p, BACKGROUND_NAMING) {
            border::Colour::Used(specified) if p.is_exhausted() => Ok(specified),

            border::Colour::Used(_) | border::Colour::Refused(_) | border::Colour::Malformed => {
                Err(())
            }
        }) {
            Ok(specified) => {
                Outcome::Value(Property::Background(BackgroundValue::Colour(specified)))
            }
            Err(()) => unhonourable(parser, "background (shorthand)"),
        },
        "background-image" => unhonourable(parser, "background-image"),

        "background-color" => {
            match parser.try_parse(|p| p.expect_ident_matching("transparent").map_err(|_| ())) {
                Ok(()) => Outcome::Value(Property::Background(BackgroundValue::Transparent)),
                Err(()) => match border::colour(parser, BACKGROUND_NAMING) {
                    border::Colour::Used(specified) => {
                        Outcome::Value(Property::Background(BackgroundValue::Colour(specified)))
                    }

                    border::Colour::Refused(what) => {
                        Outcome::Value(Property::Background(BackgroundValue::Unhonourable(what)))
                    }
                    border::Colour::Malformed => Outcome::Malformed,
                },
            }
        }
        _ => Outcome::Unknown,
    }
}
