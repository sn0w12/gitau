use serde::{Deserialize, Serialize};

use crate::engines::gix::language_stats::LanguageEntry;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteRepoInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forks: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<LanguageEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at_ms: Option<u64>,
}
