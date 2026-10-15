use super::contracts::{FlexContainerUpdate, FlexItemUpdate, Property};
use super::declarations::{Outcome, keyword, parse_length_value};

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive declaration dispatch keeps the bounded flex grammar and each named refusal visible together"
)]
pub(super) fn value_of(name: &str, parser: &mut cssparser::Parser<'_, '_>) -> Option<Outcome> {
    if let Some(outcome) = refused_alignment(name, parser) {
        return Some(outcome);
    }
    let outcome = match name {
        "flex-direction" => match keyword(parser).as_deref() {
            Some("row") => Outcome::Value(Property::FlexContainer(FlexContainerUpdate::Direction(
                crate::contracts::FlexDirection::Row,
            ))),
            Some("column") => Outcome::Value(Property::FlexContainer(
                FlexContainerUpdate::Direction(crate::contracts::FlexDirection::Column),
            )),
            Some("row-reverse" | "column-reverse") => {
                Outcome::Refused("flex-direction: reverse".to_owned())
            }
            _ => Outcome::Malformed,
        },
        "flex-wrap" => match keyword(parser).as_deref() {
            Some("nowrap") => Outcome::Value(Property::FlexContainer(FlexContainerUpdate::Nowrap)),
            Some("wrap" | "wrap-reverse") => Outcome::Refused("flex-wrap: wrap".to_owned()),
            _ => Outcome::Malformed,
        },
        "justify-content" => parse_justify(parser)
            .map(|value| Property::FlexContainer(FlexContainerUpdate::Justify(value)))
            .map_or(Outcome::Malformed, Outcome::Value),
        "align-items" => match parse_align(parser, false) {
            Ok(Some(value)) => Outcome::Value(Property::FlexContainer(
                FlexContainerUpdate::AlignItems(value),
            )),
            Ok(None) | Err(()) => Outcome::Malformed,
        },
        "align-self" => match parse_align(parser, true) {
            Ok(value) => Outcome::Value(Property::FlexItem(FlexItemUpdate::AlignSelf(value))),
            Err(()) => Outcome::Malformed,
        },
        "row-gap" => gap_outcome(parser, "row-gap", false, |row, _| {
            FlexContainerUpdate::RowGap(row)
        }),
        "column-gap" => gap_outcome(parser, "column-gap", false, |column, _| {
            FlexContainerUpdate::ColumnGap(column)
        }),
        "gap" => gap_outcome(parser, "gap", true, FlexContainerUpdate::Gap),
        "flex-grow" => parse_factor(parser)
            .map(|value| Property::FlexItem(FlexItemUpdate::Grow(value)))
            .map_or(Outcome::Malformed, Outcome::Value),
        "flex-shrink" => parse_factor(parser)
            .map(|value| Property::FlexItem(FlexItemUpdate::Shrink(value)))
            .map_or(Outcome::Malformed, Outcome::Value),
        "flex-basis" => parse_basis(parser)
            .map(|value| Property::FlexItem(FlexItemUpdate::Basis(value)))
            .map_or(Outcome::Malformed, Outcome::Value),
        "flex" => match parse_flex(parser) {
            FlexShorthand::Used {
                grow,
                shrink,
                basis,
            } => Outcome::Value(Property::FlexItem(FlexItemUpdate::Flex {
                grow,
                shrink,
                basis,
            })),
            FlexShorthand::BasisFirst => {
                Outcome::Refused("flex: basis-first multi-token form".to_owned())
            }
            FlexShorthand::Malformed => Outcome::Malformed,
        },
        "order" => match parser.expect_integer() {
            Ok(_) => Outcome::Refused("order".to_owned()),
            Err(_) => Outcome::Malformed,
        },
        _ => return None,
    };
    Some(outcome)
}

fn refused_alignment(name: &str, parser: &mut cssparser::Parser<'_, '_>) -> Option<Outcome> {
    let phrases: &[&[&str]] = match name {
        "justify-content" => &[
            &["normal"],
            &["left"],
            &["right"],
            &["stretch"],
            &["safe", "center"],
            &["unsafe", "center"],
        ],
        "align-items" | "align-self" => &[
            &["baseline"],
            &["first", "baseline"],
            &["last", "baseline"],
            &["normal"],
            &["self-start"],
            &["self-end"],
        ],
        _ => return None,
    };
    refused_phrase(parser, phrases).map(|value| Outcome::Refused(format!("{name}: {value}")))
}

fn parse_justify(parser: &mut cssparser::Parser<'_, '_>) -> Option<crate::contracts::FlexJustify> {
    use crate::contracts::FlexJustify as Value;
    match keyword(parser)?.as_str() {
        "flex-start" | "start" => Some(Value::Start),
        "flex-end" | "end" => Some(Value::End),
        "center" => Some(Value::Center),
        "space-between" => Some(Value::SpaceBetween),
        "space-around" => Some(Value::SpaceAround),
        "space-evenly" => Some(Value::SpaceEvenly),
        _ => None,
    }
}

fn refused_phrase(parser: &mut cssparser::Parser<'_, '_>, phrases: &[&[&str]]) -> Option<String> {
    for phrase in phrases {
        if parser
            .try_parse(|input| {
                for word in *phrase {
                    input.expect_ident_matching(word)?;
                }
                input.expect_exhausted()
            })
            .is_ok()
        {
            return Some(phrase.join(" "));
        }
    }
    None
}

fn gap_outcome(
    parser: &mut cssparser::Parser<'_, '_>,
    name: &str,
    allow_pair: bool,
    update: impl FnOnce(crate::contracts::Spacing, crate::contracts::Spacing) -> FlexContainerUpdate,
) -> Outcome {
    let start = parser.state();
    let mut contains_percentage = false;
    let mut contains_rem = false;
    while let Ok(token) = parser.next() {
        contains_percentage |= matches!(token, cssparser::Token::Percentage { .. });
        contains_rem |= matches!(
            token,
            cssparser::Token::Dimension { unit, .. } if unit.eq_ignore_ascii_case("rem")
        );
    }
    parser.reset(&start);
    if contains_percentage {
        return Outcome::Refused(format!("{name}: percentage"));
    }

    if contains_rem {
        return Outcome::Refused(format!("{name}: rem"));
    }
    let parsed = if allow_pair {
        parse_gap(parser)
    } else {
        gap_value(parser).map(|value| (value, value)).ok_or(())
    };
    match parsed {
        Ok((row, column)) => Outcome::Value(Property::FlexContainer(update(row, column))),
        Err(()) => Outcome::Malformed,
    }
}

fn parse_align(
    parser: &mut cssparser::Parser<'_, '_>,
    allow_auto: bool,
) -> Result<Option<crate::contracts::FlexAlign>, ()> {
    use crate::contracts::FlexAlign as Value;
    match keyword(parser).as_deref() {
        Some("auto") if allow_auto => Ok(None),
        Some("stretch") => Ok(Some(Value::Stretch)),
        Some("flex-start" | "start") => Ok(Some(Value::Start)),
        Some("flex-end" | "end") => Ok(Some(Value::End)),
        Some("center") => Ok(Some(Value::Center)),
        _ => Err(()),
    }
}

fn parse_factor(parser: &mut cssparser::Parser<'_, '_>) -> Option<crate::contracts::FlexFactor> {
    crate::contracts::FlexFactor::new(parser.expect_number().ok()?)
}

fn non_negative(spacing: crate::contracts::Spacing) -> Option<crate::contracts::Spacing> {
    (spacing.points >= 0.0 && spacing.em >= 0.0).then_some(spacing)
}

fn gap_value(parser: &mut cssparser::Parser<'_, '_>) -> Option<crate::contracts::Spacing> {
    if parser
        .try_parse(|input| input.expect_ident_matching("normal"))
        .is_ok()
    {
        return Some(crate::contracts::Spacing::points(0.0));
    }
    non_negative(parser.try_parse(parse_length_value).ok()?)
}

fn parse_gap(
    parser: &mut cssparser::Parser<'_, '_>,
) -> Result<(crate::contracts::Spacing, crate::contracts::Spacing), ()> {
    let row = gap_value(parser).ok_or(())?;
    let column = parser
        .try_parse(|input| gap_value(input).ok_or(()))
        .unwrap_or(row);
    Ok((row, column))
}

fn parse_basis(parser: &mut cssparser::Parser<'_, '_>) -> Option<crate::contracts::FlexBasis> {
    use crate::contracts::FlexBasis as Basis;
    if parser
        .try_parse(|input| input.expect_ident_matching("auto"))
        .is_ok()
    {
        return Some(Basis::Auto);
    }
    if parser
        .try_parse(|input| input.expect_ident_matching("content"))
        .is_ok()
    {
        return Some(Basis::Content);
    }
    if let Ok(value) = parser.try_parse(cssparser::Parser::expect_percentage) {
        return (value >= 0.0).then_some(Basis::Percent(value));
    }
    non_negative(parser.try_parse(parse_length_value).ok()?).map(Basis::Fixed)
}

enum FlexShorthand {
    Used {
        grow: crate::contracts::FlexFactor,
        shrink: crate::contracts::FlexFactor,
        basis: crate::contracts::FlexBasis,
    },
    BasisFirst,
    Malformed,
}

fn parse_flex(parser: &mut cssparser::Parser<'_, '_>) -> FlexShorthand {
    use crate::contracts::{FlexBasis as Basis, FlexFactor as Factor};
    if parser
        .try_parse(|input| input.expect_ident_matching("none"))
        .is_ok()
    {
        return FlexShorthand::Used {
            grow: Factor::ZERO,
            shrink: Factor::ZERO,
            basis: Basis::Auto,
        };
    }

    let start = parser.state();
    let Some(grow) = parse_factor(parser) else {
        parser.reset(&start);
        return if parse_basis(parser).is_some() && !parser.is_exhausted() {
            FlexShorthand::BasisFirst
        } else {
            parser.reset(&start);
            match parse_basis(parser) {
                Some(basis) => FlexShorthand::Used {
                    grow: Factor::ONE,
                    shrink: Factor::ONE,
                    basis,
                },
                None => FlexShorthand::Malformed,
            }
        };
    };

    let shrink = parser
        .try_parse(|input| parse_factor(input).ok_or(()))
        .unwrap_or(Factor::ONE);
    let basis = parser
        .try_parse(|input| parse_basis(input).ok_or(()))
        .unwrap_or(Basis::Fixed(crate::contracts::Spacing::points(0.0)));
    FlexShorthand::Used {
        grow,
        shrink,
        basis,
    }
}
