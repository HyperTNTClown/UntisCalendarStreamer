use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub calendar_entries: Vec<CalendarEntry>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEntry {
    pub id: i64,
    pub previous_id: Option<i64>,
    pub next_id: Option<i64>,
    pub absence_reason_id: Value,
    pub booking: Value,
    pub color: String,
    pub end_date_time: String,
    pub exam: Value,
    pub homeworks: Vec<Homework>,
    pub klasses: Vec<Klass>,
    pub lesson: Lesson,
    pub lesson_info: Value,
    pub main_student_group: Option<MainStudentGroup>,
    pub notes_all: Value,
    pub notes_all_files: Vec<Value>,
    pub notes_staff: Value,
    pub notes_staff_files: Vec<Value>,
    pub original_calendar_entry: Value,
    pub permissions: Vec<String>,
    pub resources: Vec<Value>,
    pub rooms: Vec<Room>,
    pub single_entries: Vec<SingleEntry>,
    pub start_date_time: String,
    pub status: Status,
    pub students: Vec<Value>,
    pub sub_type: Option<SubType>,
    pub subject: Subject,
    pub subst_text: Value,
    pub teachers: Vec<Teacher>,
    pub teaching_content: Option<String>,
    pub teaching_content_files: Vec<Value>,
    #[serde(rename = "type")]
    pub type_field: Type,
    pub video_call: Value,
    pub integrations_section: Vec<Value>,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    #[default]
    #[serde(rename = "TAKING_PLACE")]
    TakingPlace,
    #[serde(rename = "CANCELLED")]
    Cancelled,
    #[serde(rename = "MOVED")]
    Moved,
    #[serde(rename = "SUBSTITUTION")]
    Substitution,
    #[serde(rename = "REMOVED")]
    Removed,
    #[serde(rename = "REGULAR")]
    Regular,
    Default(String),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    #[default]
    #[serde(rename = "NORMAL_TEACHING_PERIOD")]
    NormalTeachingPeriod,
    #[serde(rename = "ADDITIONAL_PERIOD")]
    AddiotionalPeriod,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Homework {
    pub attachments: Vec<Value>,
    pub completed: bool,
    pub date_time: String,
    pub due_date_time: String,
    pub id: i64,
    pub remark: String,
    pub text: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Klass {
    pub display_name: String,
    pub has_timetable: bool,
    pub id: i64,
    pub long_name: String,
    pub short_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub lesson_id: i64,
    pub lesson_number: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainStudentGroup {
    pub id: i64,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    pub display_name: String,
    pub has_timetable: bool,
    pub id: i64,
    pub long_name: String,
    pub short_name: String,
    pub status: Status,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleEntry {
    pub id: i64,
    pub previous_id: Option<i64>,
    pub next_id: Option<i64>,
    pub created_at: Value,
    pub end_date_time: String,
    pub last_update: Value,
    pub permissions: Vec<String>,
    pub start_date_time: String,
    pub teaching_content: Option<String>,
    pub teaching_content_files: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubType {
    pub display_in_period_details: bool,
    pub display_name: String,
    pub id: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    pub display_name: String,
    pub has_timetable: bool,
    pub id: i64,
    pub long_name: String,
    pub short_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Teacher {
    pub display_name: String,
    pub has_timetable: bool,
    pub id: i64,
    pub long_name: String,
    pub short_name: String,
    pub status: String,
    pub image_url: Value,
}
