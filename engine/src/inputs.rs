use crate::contracts::{
    DocumentBase, Error, InputRef, RenderHost, RenderOptions, Requester, StylesheetRef,
};

#[derive(Clone)]
pub(crate) struct AuthorSheet {
    pub(crate) origin: String,
    pub(crate) text: String,
    pub(crate) base: Option<std::path::PathBuf>,
}

pub(crate) struct Prepared {
    pub(crate) author_sheets: Vec<AuthorSheet>,
    pub(crate) caller_bytes: usize,
    pub(crate) base: DocumentBase,
}

pub(crate) fn prepare(
    inputs: &[InputRef],
    options: &RenderOptions,
    host: &impl RenderHost,
    body_bytes: usize,
) -> Result<Prepared, Error> {
    let (author_sheets, caller_bytes) = load_author_sheets(
        &options.author_stylesheets,
        host,
        body_bytes,
        options.max_input_bytes,
        options.asset_root.as_deref(),
    )?;
    let base = inputs
        .iter()
        .find_map(|input| match input {
            InputRef::Path(path) => path
                .parent()
                .map(|dir| DocumentBase::Directory(dir.to_path_buf())),
            InputRef::Inline { .. } => None,
        })
        .unwrap_or_else(|| {
            options
                .asset_root
                .clone()
                .map_or(DocumentBase::None, DocumentBase::Directory)
        });
    Ok(Prepared {
        author_sheets,
        caller_bytes,
        base,
    })
}

pub(crate) fn load_author_sheets(
    inputs: &[StylesheetRef],
    host: &impl RenderHost,
    already_spent: usize,
    limit: usize,
    inline_base: Option<&std::path::Path>,
) -> Result<(Vec<AuthorSheet>, usize), Error> {
    let mut spent = already_spent;
    let mut sheets = Vec::with_capacity(inputs.len());
    for input in inputs {
        let (bytes, origin, base) = match input {
            StylesheetRef::Inline { bytes, origin } => (
                bytes.clone(),
                origin.clone(),
                inline_base.map(std::path::Path::to_path_buf),
            ),
            StylesheetRef::Path(path) => (
                host.read_file(path, limit.saturating_sub(spent), Requester::Caller)?,
                path.display().to_string(),
                path.parent().map(std::path::Path::to_path_buf),
            ),
        };
        if bytes.len() > limit.saturating_sub(spent) {
            return Err(Error::RequiredInputTooLarge { origin, limit });
        }
        spent = spent.saturating_add(bytes.len());
        let text = String::from_utf8(bytes).map_err(|error| Error::NotUtf8 {
            origin: origin.clone(),
            position: error.utf8_error().valid_up_to(),
        })?;
        sheets.push(AuthorSheet { origin, text, base });
    }
    Ok((sheets, spent))
}
