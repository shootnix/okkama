//! Package versions and version requirements.
//!
//! A version is `MAJOR.MINOR.PATCH` (a git tag spells it `vMAJOR.MINOR.PATCH`).
//! A requirement is one of:
//!
//! - `12.3.0` — compatible with 12.3.0: `>=12.3.0, <13.0.0`; below 1.0 the
//!   minor is the breaking part: `0.3.1` means `>=0.3.1, <0.4.0`;
//! - `12.*`, `12.3.*` — any version with that prefix;
//! - `=12.3.1` — exactly that version;
//! - `*` — any version.

use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// `1.2.3` or `v1.2.3`.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut parts = text.split('.');
        let version = Version {
            major: number(parts.next()?)?,
            minor: number(parts.next()?)?,
            patch: number(parts.next()?)?,
        };
        parts.next().is_none().then_some(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn number(text: &str) -> Option<u64> {
    (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// A version requirement, with its source spelling for messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub text: String,
    low: Version,
    /// Exclusive upper bound; `None` — no bound.
    high: Option<Version>,
}

impl Requirement {
    pub fn parse(text: &str) -> Result<Requirement, String> {
        let spelled = text.trim();
        let invalid = || {
            format!(
                "invalid version requirement \"{spelled}\" (use 1.2.3, 1.*, 1.2.*, =1.2.3 or *)"
            )
        };
        let zero = Version {
            major: 0,
            minor: 0,
            patch: 0,
        };
        let (low, high) = if spelled == "*" {
            (zero, None)
        } else if let Some(exact) = spelled.strip_prefix('=') {
            let version = Version::parse(exact.trim()).ok_or_else(invalid)?;
            let next = Version {
                patch: version.patch + 1,
                ..version
            };
            (version, Some(next))
        } else if let Some(prefix) = spelled.strip_suffix(".*") {
            let parts: Vec<u64> = prefix
                .split('.')
                .map(number)
                .collect::<Option<_>>()
                .ok_or_else(invalid)?;
            match parts.as_slice() {
                [major] => (
                    Version {
                        major: *major,
                        ..zero
                    },
                    Some(Version {
                        major: major + 1,
                        ..zero
                    }),
                ),
                [major, minor] => (
                    Version {
                        major: *major,
                        minor: *minor,
                        patch: 0,
                    },
                    Some(Version {
                        major: *major,
                        minor: minor + 1,
                        patch: 0,
                    }),
                ),
                _ => return Err(invalid()),
            }
        } else {
            let version = Version::parse(spelled).ok_or_else(invalid)?;
            let high = if version.major > 0 {
                Version {
                    major: version.major + 1,
                    ..zero
                }
            } else {
                Version {
                    minor: version.minor + 1,
                    ..zero
                }
            };
            (version, Some(high))
        };
        Ok(Requirement {
            text: spelled.to_string(),
            low,
            high,
        })
    }

    pub fn matches(&self, version: &Version) -> bool {
        *version >= self.low && self.high.is_none_or(|high| *version < high)
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn req(text: &str) -> Requirement {
        Requirement::parse(text).unwrap()
    }

    #[test]
    fn versions_parse_with_or_without_v() {
        assert_eq!(v("v12.3.1"), v("12.3.1"));
        assert_eq!(v("1.10.0").to_string(), "1.10.0");
        assert!(v("1.10.0") > v("1.9.9"));
        for bad in ["1.2", "1.2.3.4", "1.x.3", "", "v", "1.2.-3"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn plain_versions_are_compatible_ranges() {
        let caret = req("12.3.0");
        assert!(caret.matches(&v("12.3.0")) && caret.matches(&v("12.9.4")));
        assert!(!caret.matches(&v("12.2.9")) && !caret.matches(&v("13.0.0")));
        let zero = req("0.3.1");
        assert!(zero.matches(&v("0.3.7")));
        assert!(!zero.matches(&v("0.4.0")) && !zero.matches(&v("0.3.0")));
    }

    #[test]
    fn wildcards_exact_and_any() {
        assert!(req("12.*").matches(&v("12.0.0")) && !req("12.*").matches(&v("13.0.0")));
        assert!(req("12.3.*").matches(&v("12.3.9")) && !req("12.3.*").matches(&v("12.4.0")));
        assert!(req("=12.3.1").matches(&v("12.3.1")) && !req("=12.3.1").matches(&v("12.3.2")));
        assert!(req("*").matches(&v("0.0.1")) && req("*").matches(&v("99.0.0")));
        for bad in ["12", "12.x", "^1.2.3", "=1.2", "1.*.*"] {
            assert!(Requirement::parse(bad).is_err(), "{bad}");
        }
    }
}
