use serde::{Deserialize, Serialize};

use crate::domain::ids::ObjectId;
use crate::domain::revisions::RefName;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamRef {
    pub remote: RefName,
    pub branch: RefName,
    pub ahead: u32,
    pub behind: u32,
    /// Commit the remote-tracking ref points at. Lets the client walk
    /// remote-only history, e.g. to preview what a force push would drop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: RefName,
    pub target: ObjectId,
    pub is_head: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream: Option<UpstreamRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub name: RefName,
    pub target: ObjectId,
    /// For annotated tags this is the tag object itself.
    pub tag_object: Option<ObjectId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagger: Option<crate::domain::commits::Signature>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_skips_none_when_absent() {
        let oid = ObjectId::from_bytes(&[2u8; 20]).unwrap();
        let info = BranchInfo {
            name: RefName::parse("main", "branch").unwrap(),
            target: oid,
            is_head: true,
            upstream: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert!(json.get("upstream").is_none());
        assert_eq!(json["isHead"], serde_json::json!(true));
    }
}
