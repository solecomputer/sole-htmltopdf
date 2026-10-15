use sole_htmltopdf_engine::{HostError, InputRef, PageSize, RenderHost, RenderOptions};
use std::path::Path;

#[derive(Debug)]
struct NoFiles;

impl RenderHost for NoFiles {
    fn read_file(
        &self,
        path: &Path,
        _max_bytes: usize,
        _requester: sole_htmltopdf_engine::Requester,
    ) -> Result<Vec<u8>, HostError> {
        Err(HostError::NotFound {
            path: path.display().to_string(),
        })
    }
}

#[test]
fn renders_html_to_pdf() -> Result<(), sole_htmltopdf_engine::Error> {
    let outcome = sole_htmltopdf_engine::render(
        &[InputRef::Inline {
            bytes: b"<!doctype html><p>Hello</p>".to_vec(),
            origin: "smoke".to_owned(),
        }],
        RenderOptions::new(PageSize::a4()),
        &NoFiles,
    )?;
    assert!(outcome.pdf.as_slice().starts_with(b"%PDF-"));
    Ok(())
}
