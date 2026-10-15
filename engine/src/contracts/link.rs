use std::sync::Arc;

use super::Warning;

pub(crate) const MAX_LINK_TARGET_BYTES: usize = 8_192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkTarget(Arc<str>);

impl LinkTarget {
    pub(crate) fn validate(
        source: &str,
        policy: LinkPolicy,
        refusals: &mut LinkRefusals,
    ) -> Option<Self> {
        match policy {
            LinkPolicy::External => {}
            LinkPolicy::ProfileUnavailable => {
                refusals.profile_unavailable = refusals.profile_unavailable.saturating_add(1);
                return None;
            }
            LinkPolicy::FurnitureUnavailable => {
                refusals.furniture_unavailable = refusals.furniture_unavailable.saturating_add(1);
                return None;
            }
        }

        if source.len() > MAX_LINK_TARGET_BYTES {
            refusals.oversized = refusals.oversized.saturating_add(1);
            return None;
        }
        let source = source.trim_matches(|character: char| character.is_ascii_whitespace());
        if source.is_empty() {
            refusals.relative = refusals.relative.saturating_add(1);
            return None;
        }

        if source.starts_with('#') {
            refusals.fragment = refusals.fragment.saturating_add(1);
            return None;
        }
        if source.starts_with("//") {
            refusals.scheme_relative = refusals.scheme_relative.saturating_add(1);
            return None;
        }

        let parsed = match url::Url::parse(source) {
            Ok(parsed) => parsed,
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                refusals.relative = refusals.relative.saturating_add(1);
                return None;
            }
            Err(_) => {
                refusals.malformed = refusals.malformed.saturating_add(1);
                return None;
            }
        };
        if !matches!(parsed.scheme(), "http" | "https" | "mailto") {
            refusals.disallowed_scheme = refusals.disallowed_scheme.saturating_add(1);
            return None;
        }
        Some(Self(Arc::from(parsed.as_str())))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkPolicy {
    External,

    ProfileUnavailable,

    FurnitureUnavailable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LinkRefusals {
    fragment: usize,
    relative: usize,
    scheme_relative: usize,
    disallowed_scheme: usize,
    malformed: usize,
    oversized: usize,
    profile_unavailable: usize,
    furniture_unavailable: usize,
}

impl LinkRefusals {
    pub(crate) fn merge(&mut self, other: &Self) {
        self.fragment = self.fragment.saturating_add(other.fragment);
        self.relative = self.relative.saturating_add(other.relative);
        self.scheme_relative = self.scheme_relative.saturating_add(other.scheme_relative);
        self.disallowed_scheme = self
            .disallowed_scheme
            .saturating_add(other.disallowed_scheme);
        self.malformed = self.malformed.saturating_add(other.malformed);
        self.oversized = self.oversized.saturating_add(other.oversized);
        self.profile_unavailable = self
            .profile_unavailable
            .saturating_add(other.profile_unavailable);
        self.furniture_unavailable = self
            .furniture_unavailable
            .saturating_add(other.furniture_unavailable);
    }

    pub(crate) fn into_warning(self) -> Option<Warning> {
        let warning = Warning::LinksNotCreated {
            fragment: self.fragment,
            relative: self.relative,
            scheme_relative: self.scheme_relative,
            disallowed_scheme: self.disallowed_scheme,
            malformed: self.malformed,
            oversized: self.oversized,
            profile_unavailable: self.profile_unavailable,
            furniture_unavailable: self.furniture_unavailable,
        };
        (self != Self::default()).then_some(warning)
    }
}
