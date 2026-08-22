//! Explicit, repository-scoped links to GitHub issues and pull requests.
//!
//! This module intentionally accepts only structured references supplied by a
//! caller. It does not inspect titles, messages, branch names, or assistant
//! prose to guess a link.

use std::{fmt, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubRepository {
    pub owner: String,
    pub name: String,
}

impl GitHubRepository {
    pub fn new(owner: &str, name: &str) -> Option<Self> {
        if valid_segment(owner) && valid_segment(name) {
            Some(Self {
                owner: owner.to_owned(),
                name: name.to_owned(),
            })
        } else {
            None
        }
    }

    fn path(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

impl FromStr for GitHubRepository {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('/');
        let owner = parts.next().ok_or(())?;
        let name = parts.next().ok_or(())?;
        if parts.next().is_some() {
            return Err(());
        }
        Self::new(owner, name).ok_or(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitHubReferenceKind {
    Issue,
    PullRequest,
}

impl GitHubReferenceKind {
    fn path_segment(self) -> &'static str {
        match self {
            Self::Issue => "issues",
            Self::PullRequest => "pull",
        }
    }

    fn shorthand_marker(self) -> char {
        match self {
            Self::Issue => '#',
            Self::PullRequest => '!',
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubReference {
    pub repository: GitHubRepository,
    pub kind: GitHubReferenceKind,
    pub number: u64,
}

impl GitHubReference {
    pub fn new(
        repository: GitHubRepository,
        kind: GitHubReferenceKind,
        number: u64,
    ) -> Option<Self> {
        (number > 0).then_some(Self {
            repository,
            kind,
            number,
        })
    }

    /// Parse a repository-qualified reference such as `owner/repo#42` or
    /// `owner/repo!42`. Bare `#42` and `!42` references are intentionally not
    /// accepted because they are ambiguous across repositories.
    pub fn parse_qualified(value: &str) -> Option<Self> {
        let marker_position = value
            .bytes()
            .position(|byte| byte == b'#' || byte == b'!')?;
        let repository = &value[..marker_position];
        let marker = value.as_bytes().get(marker_position).copied()? as char;
        let number = value.get(marker_position + 1..)?.parse().ok()?;
        let kind = match marker {
            '#' => GitHubReferenceKind::Issue,
            '!' => GitHubReferenceKind::PullRequest,
            _ => return None,
        };
        Self::new(repository.parse().ok()?, kind, number)
    }

    /// Parse only canonical `github.com` issue or pull-request URLs.
    pub fn parse_url(value: &str) -> Option<Self> {
        let path = value.strip_prefix("https://github.com/")?;
        if path.contains('?') || path.contains('#') || path.ends_with('/') {
            return None;
        }
        let mut parts = path.split('/');
        let repository = format!("{}/{}", parts.next()?, parts.next()?);
        let kind = match parts.next()? {
            "issues" => GitHubReferenceKind::Issue,
            "pull" => GitHubReferenceKind::PullRequest,
            _ => return None,
        };
        let number = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Self::new(repository.parse().ok()?, kind, number)
    }

    pub fn url(&self) -> String {
        format!(
            "https://github.com/{}/{}/{}",
            self.repository.path(),
            self.kind.path_segment(),
            self.number
        )
    }
}

/// The explicit input form that supplied a repository-scoped reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkSource {
    QualifiedShorthand,
    CanonicalUrl,
}

/// Informational validation state for a parsed link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkValidation {
    Validated,
    Unvalidated,
}

/// A GitHub reference together with its explicit source and validation state.
///
/// This wrapper does not claim that a repository or numbered item exists on
/// GitHub. `Validated` means only that the supplied structured form passed the
/// conservative repository-scoped parser.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitHubLink {
    pub reference: GitHubReference,
    pub source: LinkSource,
    pub validation: LinkValidation,
}

impl GitHubLink {
    pub fn new(reference: GitHubReference, source: LinkSource, validation: LinkValidation) -> Self {
        Self {
            reference,
            source,
            validation,
        }
    }

    pub fn parse_qualified(value: &str) -> Option<Self> {
        Some(Self::new(
            GitHubReference::parse_qualified(value)?,
            LinkSource::QualifiedShorthand,
            LinkValidation::Validated,
        ))
    }

    pub fn parse_url(value: &str) -> Option<Self> {
        Some(Self::new(
            GitHubReference::parse_url(value)?,
            LinkSource::CanonicalUrl,
            LinkValidation::Validated,
        ))
    }

    pub fn url(&self) -> String {
        self.reference.url()
    }
}

impl fmt::Display for GitHubLink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reference.fmt(formatter)
    }
}

impl fmt::Display for GitHubReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{}{}",
            self.repository.path(),
            self.kind.shorthand_marker(),
            self.number
        )
    }
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::{
        GitHubLink, GitHubReference, GitHubReferenceKind, GitHubRepository, LinkSource,
        LinkValidation,
    };
    use std::str::FromStr;

    #[test]
    fn qualified_issue_and_pull_request_references_are_deterministic() {
        let issue = GitHubReference::parse_qualified("miki-thecat/agent-hud#85").unwrap();
        let pull_request = GitHubReference::parse_qualified("miki-thecat/agent-hud!113").unwrap();

        assert_eq!(issue.kind, GitHubReferenceKind::Issue);
        assert_eq!(issue.to_string(), "miki-thecat/agent-hud#85");
        assert_eq!(
            issue.url(),
            "https://github.com/miki-thecat/agent-hud/issues/85"
        );
        assert_eq!(pull_request.kind, GitHubReferenceKind::PullRequest);
        assert_eq!(
            pull_request.url(),
            "https://github.com/miki-thecat/agent-hud/pull/113"
        );
    }

    #[test]
    fn bare_or_prose_references_are_not_guessed() {
        for value in ["#85", "!113", "fixes #85", "agent-hud#85"] {
            assert!(GitHubReference::parse_qualified(value).is_none(), "{value}");
        }
    }

    #[test]
    fn repository_identity_is_part_of_reference_equality_and_url() {
        let first = GitHubReference::parse_qualified("one/project#7").unwrap();
        let second = GitHubReference::parse_qualified("two/project#7").unwrap();

        assert_ne!(first, second);
        assert_ne!(first.url(), second.url());
    }

    #[test]
    fn canonical_urls_round_trip_and_reject_other_routes() {
        for value in [
            "https://github.com/one/project/issues/7",
            "https://github.com/one/project/pull/8",
        ] {
            let reference = GitHubReference::parse_url(value).unwrap();
            assert_eq!(
                GitHubReference::parse_url(&reference.url()),
                Some(reference)
            );
        }
        for value in [
            "http://github.com/one/project/issues/7",
            "https://github.com/one/project/issues/7/extra",
            "https://github.com/one/project/commit/7",
            "https://github.com/one/project/issues/0",
        ] {
            assert!(GitHubReference::parse_url(value).is_none(), "{value}");
        }
    }

    #[test]
    fn repositories_require_exactly_two_valid_segments() {
        assert_eq!(
            GitHubRepository::from_str("one/project").unwrap(),
            GitHubRepository {
                owner: "one".into(),
                name: "project".into(),
            }
        );
        for value in [
            "project",
            "one/project/extra",
            "one/project name",
            "/project",
        ] {
            assert!(GitHubRepository::from_str(value).is_err(), "{value}");
        }
    }

    #[test]
    fn links_preserve_explicit_source_and_validation_metadata() {
        let shorthand = GitHubLink::parse_qualified("one/project#7").unwrap();
        assert_eq!(shorthand.source, LinkSource::QualifiedShorthand);
        assert_eq!(shorthand.validation, LinkValidation::Validated);
        assert_eq!(shorthand.to_string(), "one/project#7");

        let url = GitHubLink::parse_url("https://one.invalid/project/issues/7");
        assert!(url.is_none(), "only canonical github.com URLs are accepted");

        let canonical = GitHubLink::parse_url("https://github.com/one/project/issues/7").unwrap();
        assert_eq!(canonical.source, LinkSource::CanonicalUrl);
        assert_eq!(canonical.validation, LinkValidation::Validated);
        assert_eq!(canonical.url(), "https://github.com/one/project/issues/7");
    }

    #[test]
    fn metadata_can_record_an_unvalidated_reference_without_changing_parsing() {
        let reference = GitHubReference::parse_qualified("one/project#7").unwrap();
        let link = GitHubLink::new(
            reference,
            LinkSource::QualifiedShorthand,
            LinkValidation::Unvalidated,
        );

        assert_eq!(link.validation, LinkValidation::Unvalidated);
        assert_eq!(link.to_string(), "one/project#7");
    }
}
