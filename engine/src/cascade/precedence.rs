use super::{Declared, Node, css};

pub(super) const fn tier(origin: css::Origin, important: bool) -> u8 {
    match (origin, important) {
        (css::Origin::UserAgent, false) => 0,
        (css::Origin::Author, false) => 1,
        (css::Origin::Author, true) => 2,
        (css::Origin::UserAgent, true) => 3,
    }
}

pub(super) fn ordered_declarations<'a>(
    node: &std::rc::Rc<Node>,
    rules: &'a [css::Rule],
    inline: Option<&'a css::Parsed>,
) -> Vec<&'a Declared> {
    let mut candidates: Vec<(u8, u32, usize, &Declared)> = Vec::new();
    for (position, rule) in rules.iter().enumerate() {
        let Some(specificity) = crate::selector::match_specificity(&rule.selectors, node) else {
            continue;
        };
        for declared in &rule.declarations {
            candidates.push((
                tier(rule.origin, declared.important),
                specificity,
                position,
                declared,
            ));
        }
    }

    for declared in inline.iter().flat_map(|parsed| &parsed.declarations) {
        candidates.push((
            tier(css::Origin::Author, declared.important),
            u32::MAX,
            usize::MAX,
            declared,
        ));
    }
    candidates.sort_by_key(|(tier, specificity, position, _)| (*tier, *specificity, *position));
    candidates
        .into_iter()
        .map(|(_, _, _, declared)| declared)
        .collect()
}
