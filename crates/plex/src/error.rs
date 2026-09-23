use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("not authorized (token missing, expired or lacking access)")]
    Unauthorized,

    #[error("{method} {path} failed with HTTP {status}{}", Snippet(.body))]
    Status {
        method: String,
        path: String,
        status: u16,
        body: String,
    },

    #[error("could not decode response from {path}: {source}")]
    Decode {
        path: String,
        #[source]
        source: serde_path_to_error::Error<serde_json::Error>,
    },

    #[error("no reachable connection for server \"{0}\"")]
    NoConnection(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Status { status, .. } => Some(*status),
            Error::Unauthorized => Some(401),
            Error::Http(e) => e.status().map(|s| s.as_u16()),
            _ => None,
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }
}

struct Snippet<'a>(&'a str);

impl fmt::Display for Snippet<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.0.trim();
        if text.is_empty() || text.starts_with('<') {
            return Ok(());
        }
        let cut: String = text.chars().take(200).collect();
        write!(f, ": {cut}")
    }
}
