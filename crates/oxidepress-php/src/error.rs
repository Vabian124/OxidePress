// Modified by Vabian124 on 2026-11-07 to run WordPress plugins in a PHP worker.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use std::fmt;

/// Failure from the PHP worker, its protocol, or a plugin it included.
#[derive(Debug)]
pub enum PhpError {
    Io(std::io::Error),
    Protocol(String),
    Plugin { path: String, message: String },
    Worker(String),
    WorkerExited,
    Poisoned,
}

impl fmt::Display for PhpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhpError::Io(err) => write!(f, "php worker io error: {err}"),
            PhpError::Protocol(message) => write!(f, "php worker protocol error: {message}"),
            PhpError::Plugin { path, message } => {
                write!(f, "php plugin {path} failed: {message}")
            }
            PhpError::Worker(message) => write!(f, "php worker failed: {message}"),
            PhpError::WorkerExited => write!(f, "php worker exited"),
            PhpError::Poisoned => write!(f, "php worker lock poisoned"),
        }
    }
}

impl std::error::Error for PhpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PhpError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PhpError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            PhpError::WorkerExited
        } else {
            PhpError::Io(err)
        }
    }
}
