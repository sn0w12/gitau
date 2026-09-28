use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenRepositoryRequest {
    /// Filesystem path pointing inside a repository (searches upward).
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepositoryRequest {
    /// Existing directory the repository folder lives in, or the directory a
    /// new folder is created inside when `name` is set.
    pub parent_directory: String,
    /// Folder to create inside `parent_directory`; empty initializes
    /// `parent_directory` itself.
    pub name: String,
    pub readme: bool,
    pub gitignore_template: Option<String>,
    pub license: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitignoreTemplateInfo {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseTemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}
