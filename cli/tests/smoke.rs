use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sole-htmltopdf")
}

#[test]
fn converts_a_file() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let input = directory.join("smoke-input.html");
    let output = directory.join("smoke-output.pdf");
    std::fs::write(&input, "<!doctype html><p>Hello</p>")?;
    assert!(
        Command::new(binary())
            .arg(&input)
            .arg(&output)
            .status()?
            .success()
    );
    assert!(std::fs::read(output)?.starts_with(b"%PDF-"));
    Ok(())
}

#[test]
fn converts_stdin_to_stdout() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(binary())
        .args(["-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("stdin pipe unavailable"))?
        .write_all(b"<!doctype html><p>Hello</p>")?;
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    assert!(output.stdout.starts_with(b"%PDF-"));
    Ok(())
}

#[derive(Debug)]
struct NoFiles;

impl sole_htmltopdf::RenderHost for NoFiles {
    fn read_file(
        &self,
        path: &Path,
        _max_bytes: usize,
        _requester: sole_htmltopdf::Requester,
    ) -> Result<Vec<u8>, sole_htmltopdf::HostError> {
        Err(sole_htmltopdf::HostError::NotFound {
            path: path.display().to_string(),
        })
    }
}

#[test]
fn facade_import_renders() -> Result<(), sole_htmltopdf::Error> {
    let output = sole_htmltopdf::render(
        &[sole_htmltopdf::InputRef::Inline {
            bytes: b"<p>Facade</p>".to_vec(),
            origin: "smoke".to_owned(),
        }],
        sole_htmltopdf::RenderOptions::new(sole_htmltopdf::PageSize::a4()),
        &NoFiles,
    )?;
    assert!(output.pdf.as_slice().starts_with(b"%PDF-"));
    Ok(())
}
