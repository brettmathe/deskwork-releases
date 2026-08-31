use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub filename: String,
    pub title: String,
    pub added: Option<String>,
    pub is_slack: bool,
    pub preview: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemFull {
    pub filename: String,
    pub title: String,
    pub added: Option<String>,
    pub is_slack: bool,
    pub content: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtraEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub dir: String,
    pub name: String,
    pub priority: Option<u32>,
    /// Status as written in the projects/README.md registry table.
    pub registry_status: Option<String>,
    pub stakeholders: Option<String>,
    pub target: Option<String>,
    /// Status from the project's own README metadata block (may drift from registry).
    pub file_status: Option<String>,
    pub on_hold: bool,
    /// Folder exists but has no row in the registry table.
    pub unregistered: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetaEntry {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub rel_path: String,
    pub kind: String, // "md" | "html" | "other"
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub dir: String,
    pub name: String,
    pub readme: String,
    pub metadata: Vec<MetaEntry>,
    pub artifacts: Vec<Artifact>,
}
