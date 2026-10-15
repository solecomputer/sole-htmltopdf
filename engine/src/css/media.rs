use super::contracts::Sampled;
use crate::contracts::Evaluated;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Admits {
    Yes,

    No,
}

pub(crate) fn admits_quietly(list: &str, medium: Evaluated) -> Admits {
    admits(
        list,
        medium,
        &mut Sampled::default(),
        &mut Sampled::default(),
    )
}

pub(crate) fn admits(
    list: &str,
    medium: Evaluated,
    unsupported: &mut Sampled,
    malformed: &mut Sampled,
) -> Admits {
    let without = super::scan::without_comments(list);
    let list = without.trim();

    if list.is_empty() {
        return Admits::Yes;
    }

    let mut aside_unsupported = Sampled::default();
    let mut aside_malformed = Sampled::default();
    let mut any = false;
    for member in split_top_level(list) {
        if evaluate(
            member.trim(),
            medium,
            &mut aside_unsupported,
            &mut aside_malformed,
        ) {
            any = true;
        }
    }
    if any {
        return Admits::Yes;
    }
    unsupported.merge(&aside_unsupported);
    malformed.merge(&aside_malformed);
    Admits::No
}

fn evaluate(
    member: &str,
    medium: Evaluated,
    unsupported: &mut Sampled,
    malformed: &mut Sampled,
) -> bool {
    if member.is_empty() {
        malformed.record("<empty media query>");
        return false;
    }

    if member.matches('(').count() != member.matches(')').count() {
        malformed.record(member);
        return false;
    }
    let has_feature = member.contains('(');
    if has_feature && let Some(decided) = decided_by_type_alone(member, medium) {
        return decided;
    }
    if has_feature {
        unsupported.record(member);
        return false;
    }
    let mut words = member.split_whitespace();
    let Some(first) = words.next() else {
        malformed.record("<empty media query>");
        return false;
    };

    let qualifier = first.eq_ignore_ascii_case("not") || first.eq_ignore_ascii_case("only");
    let (negated, name) = if qualifier {
        let Some(name) = words.next() else {
            malformed.record(member);
            return false;
        };
        (first.eq_ignore_ascii_case("not"), name)
    } else {
        (false, first)
    };

    if words.next().is_some() {
        malformed.record(member);
        return false;
    }
    if !is_identifier(name) {
        malformed.record(member);
        return false;
    }
    let matches = matches_type(name, medium);
    if negated { !matches } else { matches }
}

fn decided_by_type_alone(member: &str, medium: Evaluated) -> Option<bool> {
    let mut words = member.split_whitespace();
    let first = words.next()?;

    if first.starts_with('(') {
        return None;
    }

    let negated = first.eq_ignore_ascii_case("not");
    let name = if negated || first.eq_ignore_ascii_case("only") {
        words.next()?
    } else {
        first
    };
    if name.starts_with('(') || name.eq_ignore_ascii_case("all") {
        return None;
    }

    (!matches_type(name, medium)).then_some(negated)
}

fn is_identifier(name: &str) -> bool {
    let ident_char = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii();
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| !first.is_ascii_digit() && ident_char(first))
        && characters.all(ident_char)
}

fn matches_type(name: &str, medium: Evaluated) -> bool {
    if name.eq_ignore_ascii_case("all") {
        return true;
    }
    match medium {
        Evaluated::Print => name.eq_ignore_ascii_case("print"),
        Evaluated::Screen => name.eq_ignore_ascii_case("screen"),
    }
}

fn split_top_level(list: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0_usize;
    for (index, character) in list.char_indices() {
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(list.get(start..index).unwrap_or_default());
                start = index.saturating_add(1);
            }
            _ => {}
        }
    }
    out.push(list.get(start..).unwrap_or_default());
    out
}
