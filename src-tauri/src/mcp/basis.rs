//! P1 prepares the shared data-basis types. P2 must create and validate real
//! versions/snapshots before exposing stable pagination; these types create no data.
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataVersion {
    pub database_uuid: String,
    pub data_generation: u64,
    pub commit_sequence: u64,
    pub schema_version: u32,
    pub query_schema_version: u32,
    pub statistics_definition_version: u32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuerySnapshot {
    pub id: String,
    pub client_id: String,
    pub authorization_revision: u64,
    pub filter_hash: String,
    pub projection: Vec<String>,
    pub data_version: DataVersion,
    pub ordered_stable_keys: Vec<i64>,
    pub expires_at: String,
    pub complete: bool,
    pub known_gaps: Vec<String>,
}
