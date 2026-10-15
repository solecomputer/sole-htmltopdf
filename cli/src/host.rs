use std::io::Read;
use std::path::{Path, PathBuf};

use sole_htmltopdf::{HostError, RenderHost, Requester};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalFiles {
    Denied,

    Allowed,
}

#[derive(Debug)]
pub(crate) struct NativeHost {
    source_directory: Option<PathBuf>,
    asset_root: Option<PathBuf>,
    local_files: LocalFiles,
}

impl NativeHost {
    pub(crate) fn new(
        source_directory: Option<PathBuf>,
        asset_root: Option<PathBuf>,
        local_files: LocalFiles,
    ) -> Self {
        Self {
            source_directory,
            asset_root,
            local_files,
        }
    }

    fn confinement(&self) -> Option<&Path> {
        self.asset_root.as_deref().or_else(|| {
            (self.local_files == LocalFiles::Allowed)
                .then_some(self.source_directory.as_deref())
                .flatten()
        })
    }

    fn resolves_inside(&self, path: &Path) -> bool {
        let Some(base) = self.confinement() else {
            return false;
        };
        let (Ok(real), Ok(real_base)) = (path.canonicalize(), base.canonicalize()) else {
            return false;
        };
        real.starts_with(real_base)
    }
}

impl RenderHost for NativeHost {
    #[allow(clippy::wildcard_enum_match_arm)]
    fn read_file(
        &self,
        path: &Path,
        max_bytes: usize,
        requester: Requester,
    ) -> Result<Vec<u8>, HostError> {
        let shown = path.display().to_string();

        if requester == Requester::Document {
            let Some(root) = self.confinement() else {
                return Err(HostError::AccessDisabled { path: shown });
            };
            if !self.resolves_inside(path) {
                return Err(HostError::OutsideRoot {
                    path: shown,
                    root: root.display().to_string(),
                });
            }
        }

        let read_bounded = || -> std::io::Result<Vec<u8>> {
            let file = std::fs::File::open(path)?;
            let mut buffer = Vec::new();
            file.take(bounded_take(max_bytes))
                .read_to_end(&mut buffer)?;
            Ok(buffer)
        };

        let bytes = read_bounded().map_err(|cause| match cause.kind() {
            std::io::ErrorKind::NotFound => HostError::NotFound {
                path: shown.clone(),
            },

            _ => HostError::Unreadable {
                path: shown.clone(),
                detail: cause.to_string(),
            },
        })?;

        if bytes.len() > max_bytes {
            return Err(HostError::TooLarge {
                path: shown,
                limit: max_bytes,
            });
        }
        Ok(bytes)
    }
}

fn bounded_take(limit: usize) -> u64 {
    u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1)
}
