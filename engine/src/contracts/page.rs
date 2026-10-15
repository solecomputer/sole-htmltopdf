use super::{Length, PageSize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AuthorSize {
    Page(PageSize),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageEdge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Winner<T> {
    value: T,
    important: bool,
}

impl<T: Copy> Winner<T> {
    fn replace(slot: &mut Option<Self>, value: T, important: bool) {
        if slot.is_none_or(|current| important || !current.important) {
            *slot = Some(Self { value, important });
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct AuthorPage {
    size: Option<Winner<AuthorSize>>,
    top: Option<Winner<Length>>,
    right: Option<Winner<Length>>,
    bottom: Option<Winner<Length>>,
    left: Option<Winner<Length>>,
}

impl AuthorPage {
    pub(crate) fn set_size(&mut self, value: AuthorSize, important: bool) {
        Winner::replace(&mut self.size, value, important);
    }

    pub(crate) fn set_margin(&mut self, edge: PageEdge, value: Length, important: bool) {
        let slot = match edge {
            PageEdge::Top => &mut self.top,
            PageEdge::Right => &mut self.right,
            PageEdge::Bottom => &mut self.bottom,
            PageEdge::Left => &mut self.left,
        };
        Winner::replace(slot, value, important);
    }

    pub(crate) fn merge(&mut self, later: Self) {
        if let Some(value) = later.size {
            self.set_size(value.value, value.important);
        }
        for (edge, value) in [
            (PageEdge::Top, later.top),
            (PageEdge::Right, later.right),
            (PageEdge::Bottom, later.bottom),
            (PageEdge::Left, later.left),
        ] {
            if let Some(value) = value {
                self.set_margin(edge, value.value, value.important);
            }
        }
    }

    pub(crate) fn size(self) -> Option<AuthorSize> {
        self.size.map(|winner| winner.value)
    }

    pub(crate) fn margin(self, edge: PageEdge) -> Option<Length> {
        match edge {
            PageEdge::Top => self.top,
            PageEdge::Right => self.right,
            PageEdge::Bottom => self.bottom,
            PageEdge::Left => self.left,
        }
        .map(|winner| winner.value)
    }

    pub(crate) fn is_empty(self) -> bool {
        self.size.is_none()
            && self.top.is_none()
            && self.right.is_none()
            && self.bottom.is_none()
            && self.left.is_none()
    }

    pub(crate) fn has_refusable_geometry(self) -> bool {
        matches!(self.size(), Some(AuthorSize::Page(_)))
            || self.top.is_some()
            || self.right.is_some()
            || self.bottom.is_some()
            || self.left.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PageOrientation {
    #[default]
    Portrait,
    Landscape,
}
