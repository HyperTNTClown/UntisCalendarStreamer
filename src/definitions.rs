use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub days: Vec<Day>,
    pub errors: Option<Vec<Value>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: String,
    pub resource_type: Option<String>,
    pub resource: Option<Resource>,
    pub status: String,
    pub day_entries: Option<Vec<Value>>,
    pub grid_entries: Option<Vec<GridEntry>>,
    pub back_entries: Option<Vec<Value>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub id: i64,
    pub short_name: String,
    pub long_name: String,
    pub display_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridEntry {
    pub ids: Option<Vec<i64>>,
    pub duration: Duration,
    #[serde(rename = "type")]
    pub type_field: String,
    pub status: String,
    pub status_detail: Value,
    pub name: Value,
    pub layout_start_position: i64,
    pub layout_width: i64,
    pub layout_group: i64,
    pub color: String,
    pub notes_all: String,
    pub icons: Option<Vec<String>>,
    #[serde(rename = "position1")]
    pub teacher: Option<Vec<Field>>,
    #[serde(rename = "position2")]
    pub subject: Option<Vec<Field>>,
    #[serde(default)]
    #[serde(rename = "position3")]
    pub room: Option<Vec<Field>>,
    pub position4: Value,
    pub position5: Value,
    pub texts: Option<Vec<Value>>,
    pub lesson_text: String,
    pub lesson_info: Value,
    pub substitution_text: String,
    pub user_name: Value,
    pub moved: Value,
    pub duration_total: Value,
    pub link: Value,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Duration {
    pub start: String,
    pub end: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub current: InnerField,
    pub removed: Option<InnerField>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InnerField {
    #[serde(rename = "type")]
    pub type_field: String,
    pub status: String,
    pub short_name: String,
    pub long_name: String,
    pub display_name: String,
}
