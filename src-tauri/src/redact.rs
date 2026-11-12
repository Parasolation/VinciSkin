//! Taking off the part of a value that can carry a token, before it reaches a log.
//!
//! This is not a security boundary and does not pretend to be one. What it is is what keeps a
//! diagnostic log - written to disk, kept for days, and asked for in bug reports - from
//! becoming the place a shared link's one-time code is read out of. A URL's query and fragment
//! are where such a code travels, so they are what comes off.

/// `url` with its query and fragment removed, keeping `scheme://host/path`.
///
/// A value that is not a URL is answered as it stands: the whole point is that this can be
/// applied at a log site without knowing what arrived.
#[must_use]
pub fn url(url: &str) -> &str {
    let end = url.find(['?', '#']).unwrap_or(url.len());
    &url[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_and_a_fragment_come_off_a_url() {
        assert_eq!(
            url("https://example.com/a/b.fantome?token=secret#frag"),
            "https://example.com/a/b.fantome"
        );
        assert_eq!(
            url("vinciskin://install?url=https://x/y?token=1"),
            "vinciskin://install"
        );
    }

    #[test]
    fn a_url_without_either_is_answered_as_it_stands() {
        assert_eq!(
            url("https://raw.githubusercontent.com/a/b/Annie.fantome"),
            "https://raw.githubusercontent.com/a/b/Annie.fantome"
        );
        assert_eq!(url(""), "");
        assert_eq!(url("not a url"), "not a url");
    }
}
