use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Source {
    Stdin,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Destination {
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    Portrait,
    Landscape,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per command-line switch; grouping would invent categories the grammar does not have"
)]
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct Invocation {
    pub(crate) furniture: crate::furniture::Raw,
    pub(crate) source: Source,
    pub(crate) destination: Destination,

    pub(crate) allow_local_files: bool,
    pub(crate) stylesheets: Vec<PathBuf>,
    pub(crate) asset_root: Option<PathBuf>,

    pub(crate) page_size: Option<String>,

    pub(crate) orientation: Option<Orientation>,

    pub(crate) page_width: Option<f32>,
    pub(crate) page_height: Option<f32>,

    pub(crate) margins: Margins,

    pub(crate) encoding: Option<String>,

    pub(crate) title: Option<String>,

    pub(crate) colours: sole_htmltopdf::Colours,

    pub(crate) zoom: Option<sole_htmltopdf::Zoom>,

    pub(crate) medium: sole_htmltopdf::Medium,

    pub(crate) images: sole_htmltopdf::Images,

    pub(crate) mode: sole_htmltopdf::RenderMode,

    pub(crate) quiet: bool,

    pub(crate) strict: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Margins {
    pub(crate) top: Option<f32>,
    pub(crate) bottom: Option<f32>,
    pub(crate) left: Option<f32>,
    pub(crate) right: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum Notice {
    Translated {
        flag: &'static str,
        difference: &'static str,
    },

    Unsupported {
        flag: &'static str,
    },

    UnsupportedUserStylesheet,

    TocUnsupported,

    CoverRenderedAsPage {
        path: String,
    },

    CoverSkipped {
        path: String,
    },

    BundleMemberMissing {
        member: &'static str,
    },

    InputsIgnored {
        paths: Vec<String>,
    },
}

impl std::fmt::Display for Notice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Translated { flag, difference } => {
                write!(formatter, "{flag} is accepted but {difference}")
            }
            Self::Unsupported { flag } => write!(
                formatter,
                "{flag} is recognised but not implemented in this version; \
                 it was ignored and the document was rendered without it"
            ),
            Self::UnsupportedUserStylesheet => write!(
                formatter,
                "--user-style-sheet is recognised but not implemented; use the modern repeatable --stylesheet flag for additional author CSS"
            ),
            Self::TocUnsupported => write!(
                formatter,
                "toc is recognised but this version has no table of contents; \
                 it was skipped and the document was rendered without it"
            ),
            Self::CoverRenderedAsPage { path } => write!(
                formatter,
                "cover {path} was rendered as an ordinary page: this version \
                 has no separate cover"
            ),
            Self::CoverSkipped { path } => write!(
                formatter,
                "cover {path} was NOT rendered: this version has no separate \
                 cover, and the document was rendered without it"
            ),
            Self::BundleMemberMissing { member } => write!(
                formatter,
                "--quirks: {member} is part of the wkhtmltopdf 0.12.6 bundle \
                 but is not implemented in this version; it was not applied"
            ),
            Self::InputsIgnored { paths } => write!(
                formatter,
                "only the first input is rendered in this version; ignored: {}",
                paths.join(", ")
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum Error {
    #[error(
        "usage: sole-htmltopdf <input> <output>\n  \
         <input>   file path, or - for stdin\n  \
         <output>  file path, or - for stdout"
    )]
    Usage,

    #[error("unknown option {name:?}")]
    UnknownOption { name: String },

    #[error("{name} needs {arity} value(s) and did not get them")]
    MissingValue { name: String, arity: usize },

    #[error("{name} does not take a value")]
    UnexpectedValue { name: String },

    #[error("an option token is not valid Unicode: {value:?}; use a separate path value")]
    InvalidOptionEncoding { value: String },

    #[error("{name} does not accept {value:?}; expected {expected}")]
    InvalidValue {
        name: &'static str,
        value: String,
        expected: &'static str,
    },

    #[error("{name} cannot be honoured: {reason}")]
    RefusedOption {
        name: &'static str,
        reason: &'static str,
    },

    #[error(
        "network input is not available in this version: {url}\n  \
         this build renders local files and stdin only"
    )]
    NetworkInput { url: String },

    #[error("internal: {name} is declared implemented but was not applied")]
    UnhandledImplementedFlag { name: &'static str },

    #[error("{count} option(s) were not implemented and --strict was given:\n  {details}")]
    StrictRefusal { count: usize, details: String },

    #[error("cannot read input {path}: {cause}")]
    InputUnreadable { path: String, cause: String },

    #[error("cannot use asset root {path}: {cause}")]
    AssetRootUnreadable { path: String, cause: String },

    #[error("--asset-root {root} does not contain the input directory {input_directory}")]
    AssetRootMismatch {
        root: String,
        input_directory: String,
    },

    #[error("cannot resolve input directory {path} for --asset-root confinement: {cause}")]
    InputDirectoryUnreadable { path: String, cause: String },

    #[error("cannot write output {path}: {cause}")]
    OutputUnwritable { path: String, cause: String },

    #[error("render failed: {0}")]
    Render(#[from] sole_htmltopdf::Error),
}

impl Error {
    pub(crate) const fn code(&self) -> u8 {
        match self {
            Self::Usage
            | Self::UnknownOption { .. }
            | Self::MissingValue { .. }
            | Self::UnexpectedValue { .. }
            | Self::InvalidOptionEncoding { .. }
            | Self::InvalidValue { .. }
            | Self::RefusedOption { .. }
            | Self::StrictRefusal { .. }
            | Self::AssetRootMismatch { .. } => 2,

            Self::InputUnreadable { .. }
            | Self::InputDirectoryUnreadable { .. }
            | Self::AssetRootUnreadable { .. }
            | Self::OutputUnwritable { .. }
            | Self::NetworkInput { .. }
            | Self::UnhandledImplementedFlag { .. }
            | Self::Render(_) => 1,
        }
    }
}
