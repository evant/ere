use std::path::PathBuf;
use std::process::ExitStatus;

#[derive(std::fmt::Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("failed to rename {from} -> {to}: {source}")]
    Rename {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },

    #[error("$EDITOR returned a non-zero status code {0}")]
    EditorStatus(ExitStatus),

    #[error("count does not match number of files in directory, make sure not to delete or remote lines")]
    CountMismatch,

    #[error("{}", .0.into_iter().map(| e | e.to_string()).collect::< Vec < String >> ().join("\n"))]
    Group(Vec<Error>),
}
