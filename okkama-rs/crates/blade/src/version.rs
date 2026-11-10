//! Package versions and version requirements.
//!
//! A version is `MAJOR.MINOR.PATCH` (a git tag spells it `vMAJOR.MINOR.PATCH`).
//! A requirement is a minimum, `1.2.3`: that version or a later one of the
//! same major. Resolution picks the lowest version that meets every minimum
//! of a package (#347).

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

/// A version requirement — a minimum within its major — with its source
/// spelling for messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub text: String,
    min: Version,
}

impl Requirement {
    pub fn parse(text: &str) -> Result<Requirement, String> {
        let spelled = text.trim();
        // The forms of earlier versions of Blade, with what to write instead.
        let hint = if spelled == "*" {
            " — \"*\" is gone: write the lowest version that works, such as \"1.0.0\""
        } else if spelled.starts_with('=') {
            " — exact versions are gone: write the version itself; blade.lock keeps it exact"
        } else if spelled.ends_with(".*") {
            " — wildcards are gone: \"1.*\" is \"1.0.0\", \"1.2.*\" is \"1.2.0\""
        } else {
            ""
        };
        let min = Version::parse(spelled)
            .filter(|_| hint.is_empty())
            .ok_or_else(|| {
                format!(
                    "invalid version requirement \"{spelled}\": write the minimum version as MAJOR.MINOR.PATCH \
                     (it means that version or a later one of the same major){hint}"
                )
            })?;
        Ok(Requirement {
            text: spelled.to_string(),
            min,
        })
    }

    /// The lowest version the requirement accepts.
    pub fn min(&self) -> Version {
        self.min
    }

    pub fn matches(&self, version: &Version) -> bool {
        version.major == self.min.major && *version >= self.min
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "≥ {}", self.text)
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
    fn a_requirement_is_a_minimum_within_its_major() {
        let min = req("12.3.0");
        assert!(min.matches(&v("12.3.0")) && min.matches(&v("12.9.4")));
        assert!(!min.matches(&v("12.2.9")) && !min.matches(&v("13.0.0")));
        // Major 0 is one major like any other.
        let zero = req("0.3.1");
        assert!(zero.matches(&v("0.3.7")) && zero.matches(&v("0.4.0")));
        assert!(!zero.matches(&v("0.3.0")) && !zero.matches(&v("1.0.0")));
        assert_eq!(req("v1.2.3").min(), v("1.2.3"));
        assert_eq!(min.to_string(), "≥ 12.3.0");
    }

    #[test]
    fn earlier_forms_are_refused_with_what_to_write() {
        for (bad, says) in [
            ("*", "\"*\" is gone"),
            ("=12.3.1", "exact versions"),
            ("12.*", "wildcards"),
            ("12.3.*", "wildcards"),
        ] {
            let error = Requirement::parse(bad).unwrap_err();
            assert!(
                error.contains(says) && error.contains("MAJOR.MINOR.PATCH"),
                "{bad}: {error}"
            );
        }
        for bad in ["12", "12.x", "^1.2.3", "=1.2", "1.*.*", ""] {
            assert!(Requirement::parse(bad).is_err(), "{bad}");
        }
    }
}
