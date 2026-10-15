use super::DocumentBase;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Doctype {
    pub name: String,
    pub public_id: String,
    pub system_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HtmlQuirksMode {
    NoQuirks,

    LimitedQuirks,

    Quirks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SheetRef {
    Inline { text: String, media: Option<String> },

    Linked { href: String, media: Option<String> },
}

#[derive(Debug)]
#[non_exhaustive]
pub struct Dom {
    pub doctype: Option<Doctype>,

    pub html_quirks: HtmlQuirksMode,

    pub base: DocumentBase,

    pub sheets: Vec<SheetRef>,

    pub(crate) root: std::rc::Rc<crate::dom::Node>,
}

impl Dom {
    #[must_use]
    pub fn text(&self) -> String {
        crate::parse::flatten_text(&self.root)
    }
}
