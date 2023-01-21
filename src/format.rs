//! Details of the file formats used by cargo vet

use crate::errors::VersionParseError;
use crate::resolver::{DiffRecommendation, ViolationConflict};
use crate::serialization::spanned::Spanned;
use crate::{flock::Filesystem, serialization};
use core::{cmp, fmt};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::str::FromStr;

use cargo_metadata::semver;
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

// Collections based on how we're using, so it's easier to swap them out.
pub type FastMap<K, V> = HashMap<K, V>;
pub type FastSet<T> = HashSet<T>;
pub type SortedMap<K, V> = BTreeMap<K, V>;
pub type SortedSet<T> = BTreeSet<T>;

pub type CriteriaName = String;
pub type CriteriaStr<'a> = &'a str;
pub type ForeignCriteriaName = String;
pub type PackageName = String;
pub type PackageStr<'a> = &'a str;
pub type ImportName = String;

// newtype VersionReq so that we can implement PartialOrd on it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionReq(pub semver::VersionReq);
impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl FromStr for VersionReq {
    type Err = <semver::VersionReq as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        semver::VersionReq::from_str(s).map(VersionReq)
    }
}
impl core::ops::Deref for VersionReq {
    type Target = semver::VersionReq;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl cmp::PartialOrd for VersionReq {
    fn partial_cmp(&self, other: &VersionReq) -> Option<cmp::Ordering> {
        format!("{}", self).partial_cmp(&format!("{}", other))
    }
}
impl VersionReq {
    pub fn parse(text: &str) -> Result<Self, <Self as FromStr>::Err> {
        cargo_metadata::semver::VersionReq::parse(text).map(VersionReq)
    }
    pub fn matches(&self, version: &VetVersion) -> bool {
        self.0.matches(&version.semver)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VetVersion {
    pub semver: semver::Version,
    pub git_rev: Option<String>,
}
impl VetVersion {
    pub fn parse(s: &str) -> Result<Self, VersionParseError> {
        if let Some((ver, rev)) = s.split_once('@') {
            if let Some(hash) = rev.trim_start().strip_prefix("git:") {
                Ok(VetVersion {
                    semver: ver.trim_end().parse()?,
                    git_rev: Some(hash.to_owned()),
                })
            } else {
                Err(VersionParseError::UnknownRevision)
            }
        } else {
            Ok(VetVersion {
                semver: s.parse()?,
                git_rev: None,
            })
        }
    }
}
impl fmt::Display for VetVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.git_rev {
            Some(hash) => write!(f, "{}@git:{}", self.semver, hash),
            None => self.semver.fmt(f),
        }
    }
}
impl FromStr for VetVersion {
    type Err = VersionParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}
impl Serialize for VetVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for VetVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionVisitor;

        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = VetVersion;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("semver version")
            }
            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                VetVersion::parse(string).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(VersionVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                 Metaconfigs (found in Cargo.tomls)                             //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

/// A `[*.metadata.vet]` table in a Cargo.toml, configuring our behaviour
#[derive(serde::Deserialize)]
pub struct MetaConfigInstance {
    // Reserved for future use, if not present version=1 assumed.
    // (not sure whether this versions the format, or semantics, or...
    // for now assuming this species global semantics of some kind.
    pub version: Option<u64>,
    pub store: Option<StoreInfo>,
}
#[derive(serde::Deserialize)]
pub struct StoreInfo {
    pub path: Option<PathBuf>,
}

// FIXME: It's *possible* for someone to have a workspace but not have a
// global `vet` instance for the whole workspace. In this case they *could*
// have individual `vet` instances for each subcrate they care about.
// This is... Weird, and it's unclear what that *means*... but maybe it's valid?
// Either way, we definitely don't support it right now!

/// All available configuration files, overlaying each other.
/// Generally contains: `[Default, Workspace, Package]`
pub struct MetaConfig(pub Vec<MetaConfigInstance>);

impl MetaConfig {
    pub fn store_path(&self) -> Filesystem {
        // Last config gets priority to set this
        for config in self.0.iter().rev() {
            if let Some(store) = &config.store {
                if let Some(path) = &store.path {
                    return Filesystem::new(path.into());
                }
            }
        }
        unreachable!("Default config didn't define store.path???");
    }
    pub fn version(&self) -> u64 {
        // Last config gets priority to set this
        for config in self.0.iter().rev() {
            if let Some(ver) = config.version {
                return ver;
            }
        }
        unreachable!("Default config didn't define version???");
    }
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                                audits.toml                                     //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

pub type AuditedDependencies = SortedMap<PackageName, Vec<AuditEntry>>;

/// audits.toml
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AuditsFile {
    /// A map of criteria_name to details on that criteria.
    #[serde(skip_serializing_if = "SortedMap::is_empty")]
    #[serde(default)]
    pub criteria: SortedMap<CriteriaName, CriteriaEntry>,
    /// Actual audits.
    pub audits: AuditedDependencies,
}

/// Foreign audits.toml with unparsed entries and audits. Should have the same
/// structure as `AuditsFile`, but with individual audits and criteria unparsed.
#[derive(serde::Deserialize, Clone)]
pub struct ForeignAuditsFile {
    #[serde(default)]
    pub criteria: SortedMap<CriteriaName, toml::Value>,
    pub audits: SortedMap<PackageName, Vec<toml::Value>>,
}

/// Information on a Criteria
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CriteriaEntry {
    /// Summary of how you evaluate something by this criteria.
    pub description: Option<String>,
    /// An alternative to description which locates the criteria text at a publicly-accessible URL.
    /// This can be useful for sharing criteria descriptions across multiple repositories.
    #[serde(rename = "description-url")]
    pub description_url: Option<String>,
    /// Criteria that this one implies
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    #[serde(with = "serialization::string_or_vec")]
    pub implies: Vec<Spanned<CriteriaName>>,
    /// Chain of sources this criteria was aggregated from, most recent last.
    #[serde(rename = "aggregated-from")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    #[serde(with = "serialization::string_or_vec")]
    pub aggregated_from: Vec<Spanned<String>>,
}

/// This is conceptually an enum
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(try_from = "serialization::audit::AuditEntryAll")]
#[serde(into = "serialization::audit::AuditEntryAll")]
pub struct AuditEntry {
    pub who: Vec<Spanned<String>>,
    pub criteria: Vec<Spanned<CriteriaName>>,
    pub kind: AuditKind,
    pub notes: Option<String>,
    /// Chain of sources this audit was aggregated from, most recent last.
    pub aggregated_from: Vec<Spanned<String>>,
    /// A non-serialized member which indicates whether this audit is a "fresh"
    /// audit. This will be set for all audits imported found in the remote
    /// audits file which aren't also found in the local `imports.lock` cache.
    ///
    /// This should almost always be `false`, and only set to `true` by the
    /// import handling code.
    #[serde(skip)]
    pub is_fresh_import: bool,
}

/// Implement PartialOrd manually because the order we want for sorting is
/// different than the order we want for serialization.
impl cmp::PartialOrd for AuditEntry {
    fn partial_cmp<'a>(&'a self, other: &'a AuditEntry) -> Option<cmp::Ordering> {
        let tuple = |x: &'a AuditEntry| (&x.kind, &x.criteria, &x.who, &x.notes);
        tuple(self).partial_cmp(&tuple(other))
    }
}

impl cmp::Ord for AuditEntry {
    fn cmp(&self, other: &AuditEntry) -> cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum AuditKind {
    Full { version: VetVersion },
    Delta { from: VetVersion, to: VetVersion },
    Violation { violation: VersionReq },
}

/// A "VERSION" or "VERSION -> VERSION"
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Delta {
    pub from: Option<VetVersion>,
    pub to: VetVersion,
}

impl<'de> Deserialize<'de> for Delta {
    fn deserialize<D>(deserializer: D) -> Result<Delta, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DeltaVisitor;

        impl<'de> Visitor<'de> for DeltaVisitor {
            type Value = Delta;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("version -> version delta")
            }

            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Some((from, to)) = string.split_once("->") {
                    Ok(Delta {
                        from: Some(VetVersion::parse(from.trim()).map_err(de::Error::custom)?),
                        to: VetVersion::parse(to.trim()).map_err(de::Error::custom)?,
                    })
                } else {
                    Ok(Delta {
                        from: None,
                        to: VetVersion::parse(string.trim()).map_err(de::Error::custom)?,
                    })
                }
            }
        }

        deserializer.deserialize_str(DeltaVisitor)
    }
}

impl Serialize for Delta {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.from {
            Some(from) => format!("{} -> {}", from, self.to).serialize(serializer),
            None => self.to.serialize(serializer),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                                config.toml                                     //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

/// config.toml
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ConfigFile {
    /// This top-level key specifies the default criteria that cargo vet certify will use
    /// when recording audits. If unspecified, this defaults to "safe-to-deploy".
    #[serde(rename = "default-criteria")]
    #[serde(default = "get_default_criteria")]
    #[serde(skip_serializing_if = "is_default_criteria")]
    pub default_criteria: CriteriaName,

    /// Remote audits.toml's that we trust and want to import.
    #[serde(skip_serializing_if = "SortedMap::is_empty")]
    #[serde(default)]
    pub imports: SortedMap<ImportName, RemoteImport>,

    /// A table of policies for first-party crates.
    #[serde(skip_serializing_if = "SortedMap::is_empty")]
    #[serde(default)]
    pub policy: SortedMap<PackageName, PolicyEntry>,

    /// All of the "foreign" dependencies that we rely on but haven't audited yet.
    /// Foreign dependencies are just "things on crates.io", everything else
    /// (paths, git, etc) is assumed to be "under your control" and therefore implicitly trusted.
    #[serde(skip_serializing_if = "SortedMap::is_empty")]
    #[serde(default)]
    #[serde(alias = "unaudited")]
    pub exemptions: SortedMap<PackageName, Vec<ExemptedDependency>>,
}

pub static SAFE_TO_DEPLOY: CriteriaStr = "safe-to-deploy";
pub static SAFE_TO_RUN: CriteriaStr = "safe-to-run";
pub static DEFAULT_CRITERIA: CriteriaStr = SAFE_TO_DEPLOY;

pub fn get_default_criteria() -> CriteriaName {
    CriteriaName::from(DEFAULT_CRITERIA)
}
fn is_default_criteria(val: &CriteriaName) -> bool {
    val == DEFAULT_CRITERIA
}

/// Policies that first-party (non-foreign) crates must pass.
///
/// This is basically the first-party equivalent of audits.toml, which is separated out
/// because it's not supposed to be shared (or, doesn't really make sense to share,
/// since first-party crates are defined by "not on crates.io").
///
/// Because first-party crates are implicitly trusted, really the only purpose of this
/// table is to define the boundary between a first-party crates and third-party ones.
/// More specifically, the criteria of the dependency edges between a first-party crate
/// and its direct third-party dependencies.
///
/// If this sounds overwhelming, don't worry, everything defaults to "nothing special"
/// and an empty PolicyTable basically just means "everything should satisfy the
/// default criteria in audits.toml".
#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct PolicyEntry {
    /// Whether this nominally-first-party crate should actually be subject to audits
    /// as-if it was third-party, based on matches to crates.io packages with the same
    /// name and version. This field is optional for any package that *doesn't* have
    /// such a match, and mandatory for all others (None == Some(false)).
    ///
    /// If true, this package will be handled like a third-party package and require
    /// audits. If the package is not in the crates.io registry, it will be an error
    /// and you should either make sure the current version is published or flip
    /// this back to false.
    ///
    /// Setting this value to true is intended for actual externally developed projects
    /// that you are importing into your project in a weird way with minimal modifications.
    /// For instance, if you manually vendor the package in, or maintain a small patchset
    /// on top of the currently published version.
    ///
    /// It should not be used for packages that are directly developed in this project
    /// (a project shouldn't publish audits for its own code) or for non-trivial forks.
    ///
    /// Audits you *do* perform should be for the actual version published to crates.io,
    /// which are the versions `cargo vet diff` and `cargo vet inspect` will fetch.
    #[serde(rename = "audit-as-crates-io")]
    pub audit_as_crates_io: Option<bool>,

    /// Default criteria that must be satisfied by all *direct* third-party (foreign)
    /// dependencies of first-party crates. If satisfied, the first-party crate is
    /// set to satisfying all criteria.
    ///
    /// If not present, this defaults to the default criteria in the audits table.
    #[serde(default)]
    #[serde(with = "serialization::string_or_vec_or_none")]
    pub criteria: Option<Vec<Spanned<CriteriaName>>>,

    /// Same as `criteria`, but for first-party(?) crates/dependencies that are only
    /// used as dev-dependencies.
    #[serde(rename = "dev-criteria")]
    #[serde(default)]
    #[serde(with = "serialization::string_or_vec_or_none")]
    pub dev_criteria: Option<Vec<Spanned<CriteriaName>>>,

    /// Custom criteria for a specific first-party crate's dependencies.
    ///
    /// Any dependency edge that isn't explicitly specified defaults to `criteria`.
    #[serde(rename = "dependency-criteria")]
    #[serde(skip_serializing_if = "DependencyCriteria::is_empty")]
    #[serde(with = "serialization::dependency_criteria")]
    #[serde(default)]
    pub dependency_criteria: DependencyCriteria,

    /// Freeform notes
    pub notes: Option<String>,
}

/// A list of criteria that transitive dependencies must satisfy for this
/// policy to be satisfied.
///
/// Example:
///
/// ```toml
/// dependency_criteria = { hmac = ['secure', 'crypto_reviewed'] }
/// ```
pub type DependencyCriteria = SortedMap<PackageName, Vec<Spanned<CriteriaName>>>;

pub static DEFAULT_POLICY_CRITERIA: CriteriaStr = SAFE_TO_DEPLOY;
pub static DEFAULT_POLICY_DEV_CRITERIA: CriteriaStr = SAFE_TO_RUN;

/// A remote audits.toml that we trust the contents of (by virtue of trusting the maintainer).
#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
pub struct RemoteImport {
    /// URL of the foreign audits.toml
    pub url: String,
    /// A list of crates for which no audits or violations should be imported.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub exclude: Vec<PackageName>,
    /// A list of criteria that are implied by foreign criteria
    #[serde(rename = "criteria-map")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub criteria_map: Vec<CriteriaMapping>,
}

/// Translations of foreign criteria to local criteria.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct CriteriaMapping {
    /// This local criteria is implied...
    pub ours: CriteriaName,
    /// If all of these foreign criteria apply
    #[serde(with = "serialization::string_or_vec")]
    pub theirs: Vec<Spanned<ForeignCriteriaName>>,
}

/// Semantically identical to a 'full audit' entry, but private to our project
/// and tracked as less-good than a proper audit, so that you try to get rid of it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExemptedDependency {
    /// The version of the crate that we are currently "fine" with leaving unaudited.
    pub version: VetVersion,
    /// Criteria that we're willing to handwave for this version (assuming our dependencies
    /// satisfy this criteria). This isn't defaulted, 'vet init' and similar commands will
    /// pick a "good" initial value.
    #[serde(default)]
    #[serde(with = "serialization::string_or_vec")]
    pub criteria: Vec<Spanned<CriteriaName>>,
    /// Whether 'suggest' should bother mentioning this (defaults true).
    #[serde(default = "get_default_exemptions_suggest")]
    #[serde(skip_serializing_if = "is_default_exemptions_suggest")]
    pub suggest: bool,
    /// Freeform notes, put whatever you want here. Just more stable/reliable than comments.
    pub notes: Option<String>,
}

static DEFAULT_EXEMPTIONS_SUGGEST: bool = true;
pub fn get_default_exemptions_suggest() -> bool {
    DEFAULT_EXEMPTIONS_SUGGEST
}
fn is_default_exemptions_suggest(val: &bool) -> bool {
    val == &DEFAULT_EXEMPTIONS_SUGGEST
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                                imports.lock                                    //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

/// imports.lock, not sure what I want to put in here yet.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ImportsFile {
    pub audits: SortedMap<ImportName, AuditsFile>,
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                               diffcache.toml                                   //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

/// The current DiffCache file format in a tagged enum.
///
/// If we fail to read the DiffCache it will be silently re-built, meaning that
/// the version enum tag can be changed to force the DiffCache to be
/// re-generated after a breaking change to the format, such as a change to how
/// diffs are computed or identified.
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "version")]
pub enum DiffCache {
    #[serde(rename = "2")]
    V2 {
        diffs: SortedMap<PackageName, SortedMap<Delta, DiffStat>>,
    },
}

impl Default for DiffCache {
    fn default() -> Self {
        DiffCache::V2 {
            diffs: SortedMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiffStat {
    pub insertions: u64,
    pub deletions: u64,
    pub files_changed: u64,
}

impl DiffStat {
    pub fn count(&self) -> u64 {
        self.insertions + self.deletions
    }
}

impl fmt::Display for DiffStat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} files changed", self.files_changed)?;
        if self.insertions > 0 {
            write!(f, ", {} insertions(+)", self.insertions)?;
        }
        if self.deletions > 0 {
            write!(f, ", {} deletions(-)", self.deletions)?;
        }
        Ok(())
    }
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                             command-history.json                               //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FetchCommand {
    Inspect {
        package: PackageName,
        version: VetVersion,
    },
    Diff {
        package: PackageName,
        version1: VetVersion,
        version2: VetVersion,
    },
}

impl FetchCommand {
    pub fn package(&self) -> PackageStr {
        match self {
            FetchCommand::Inspect { package, .. } => package,
            FetchCommand::Diff { package, .. } => package,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CommandHistory {
    #[serde(flatten)]
    pub last_fetch: Option<FetchCommand>,
}

////////////////////////////////////////////////////////////////////////////////////
//                                                                                //
//                                                                                //
//                                                                                //
//                             <json report output>                               //
//                                                                                //
//                                                                                //
//                                                                                //
////////////////////////////////////////////////////////////////////////////////////

/// cargo-vet's `--output-format=json` for `check` and `suggest` on:
///
/// * success
/// * audit failure
/// * violation conflicts
///
/// Other errors like i/o or supply-chain integrity issues will show
/// up as miette-style json errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReport {
    /// Additional optional context for automation. Only enabled with `--output-format=json-full`
    pub context: Option<JsonReportContext>,
    #[serde(flatten)]
    pub conclusion: JsonReportConclusion,
}

/// Additional context for automation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReportContext {
    /// Where the store (supply-chain) is located
    pub store_path: String,
    /// The currently defined criteria (currently excludes builtins criteria like `safe-to-deploy`).
    pub criteria: SortedMap<CriteriaName, CriteriaEntry>,
}

/// The conclusion of running `check` or `suggest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "conclusion")]
pub enum JsonReportConclusion {
    /// Success! Everything's Good.
    #[serde(rename = "success")]
    Success(JsonReportSuccess),
    /// The violations and audits/exemptions are contradictory!
    #[serde(rename = "fail (violation)")]
    FailForViolationConflict(JsonReportFailForViolationConflict),
    /// The audit failed, here's why and what to do.
    #[serde(rename = "fail (vetting)")]
    FailForVet(JsonReportFailForVet),
}

/// Success! Everything is audited!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReportSuccess {
    /// These packages are fully vetted
    pub vetted_fully: Vec<JsonPackage>,
    /// These packages are partially vetted (some audits but relies on an `exemption`).
    pub vetted_partially: Vec<JsonPackage>,
    /// These packages are exempted
    pub vetted_with_exemptions: Vec<JsonPackage>,
}

/// Failure! The violations and audits/exemptions are contradictory!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReportFailForViolationConflict {
    /// These packages have the following conflicts
    // FIXME(SCHEMA): we probably shouldn't expose this internal type
    pub violations: SortedMap<PackageAndVersion, Vec<ViolationConflict>>,
}

/// Failure! You need more audits!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReportFailForVet {
    /// Here are the problems we found
    pub failures: Vec<JsonVetFailure>,
    /// And here are the fixes we recommend
    pub suggest: Option<JsonSuggest>,
}

/// Suggested fixes for a FailForVet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSuggest {
    /// Here are the suggestions sorted in the order of priority
    pub suggestions: Vec<JsonSuggestItem>,
    /// The same set of suggestions but grouped by the criteria (lists) needed to audit them
    // FIXME(SCHEMA): this is kinda redundant? do consumers want this?
    pub suggest_by_criteria: SortedMap<String, Vec<JsonSuggestItem>>,
    /// The total number of lines you would need to review to resolve this
    pub total_lines: u64,
}

/// This specific package needed the following criteria but doesn't have them!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonVetFailure {
    /// The name of the package
    pub name: PackageName,
    /// The version of the package
    pub version: VetVersion,
    /// The missing criteria
    pub missing_criteria: Vec<CriteriaName>,
}

/// We recommend auditing the following package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSuggestItem {
    /// The name of the package
    pub name: PackageName,
    /// Any notable parents the package has (can be helpful in giving context to the user)
    // FIXME(SCHEMA): we probably shouldn't expose this as a String
    pub notable_parents: String,
    /// The criteria we recommend auditing the package for
    pub suggested_criteria: Vec<CriteriaName>,
    /// The diff (or full version) we recommend auditing
    // FIXME(SCHEMA): we probably shouldn't expose this internal type
    pub suggested_diff: DiffRecommendation,
    /// Whether the suggestion is confident or a guess (de-emphasize guesses)
    pub confident: bool,
}

/// A string of the form "package:version"
pub type PackageAndVersion = String;

/// A Package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPackage {
    /// Name of the package
    pub name: PackageName,
    /// Version of the package
    pub version: VetVersion,
}
