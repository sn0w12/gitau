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
    /// Existing directory the new repository folder is created inside, or the
    /// repository root itself when [`CreateRepositoryRequest::init_in_place`]
    /// is set.
    pub parent_directory: String,
    /// Name of the new repository folder; ignored when `init_in_place` is
    /// set.
    pub name: String,
    /// Initializes the repository directly in `parent_directory` instead of
    /// creating a named subfolder, allowing existing files.
    #[serde(default)]
    pub init_in_place: bool,
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
