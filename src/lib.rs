use std::collections::HashMap;

use chrono::NaiveDate;
use chrono::NaiveDateTime;
use cookie::Cookie;

/// Apparently not needed for fetching the homeworks, just need session_id cookie which you'd get from the security check
// async fn get_token(&mut self, cookies: &[Cookie<'_>]) {
//     let url = "https://nessa.webuntis.com/WebUntis/api/token/new";

//     let cookie_string = cookies
//         .iter()
//         .map(|el| el.name_value())
//         .map(|(el, el1)| format!("{el}={el1}"))
//         .reduce(|el, el1| format!("{el}; {el1}"))
//         .unwrap_or_default();

//     println!("{cookie_string}");
//     let client = reqwest::Client::new();
//     let res = client
//         .get(url)
//         .header("cookie", cookie_string)
//         .send()
//         .await
//         .unwrap();
//     println!("{res:?}");
//     let token = res.text().await.unwrap();
//     *self = WebClient::Authenticated { token }
// }

/// Looks like passing that shit from one function to the ohter would be pretty annoying cuz of lifetimes, sooo one big monster function it is...
// pub async fn auth<'a>() -> Vec<Cookie<'a>> {
//     let url = "https://nessa.webuntis.com/WebUntis/j_spring_security_check";
//     let client = reqwest::Client::new();
//     let res = client
//         .post(url)
//         .header("Content-Type", "application/x-www-form-urlencoded")
//         .header("Accept", "application/json")
//         .body("school=gymnasium+am+markt&j_username=Jahrgang12&j_password=Goofy23&token=")
//         .send()
//         .await
//         .unwrap();

//     let headers = res.headers();

//     let cookies = headers
//         .get_all("set-cookie")
//         .iter()
//         .map(|el| Cookie::parse(el.to_str().unwrap()).unwrap())
//         .collect::<Vec<_>>();
use ics::components::Property;
//     cookies
// }
use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub data: Data,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub records: Vec<Record>,
    pub homeworks: Vec<Homework>,
    pub teachers: Vec<Teacher>,
    pub lessons: Vec<Lesson>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub homework_id: i64,
    pub teacher_id: i64,
    pub element_ids: Vec<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Homework {
    pub id: i64,
    pub lesson_id: i64,
    pub date: i64,
    pub due_date: i64,
    pub text: String,
    pub remark: String,
    pub completed: bool,
    pub attachments: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Teacher {
    pub id: i64,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub id: i64,
    pub subject: String,
    pub lesson_type: String,
}

pub fn homework(start_date: &str, end_date: &str) -> HashMap<String, Vec<ics::Event<'static>>> {
    let url = "https://nessa.webuntis.com/WebUntis/j_spring_security_check";
    let client = reqwest::blocking::Client::new();
    let res = client
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .body("school=gymnasium+am+markt&j_username=Jahrgang12&j_password=Goofy23&token=")
        .send()
        .unwrap();

    let headers = res.headers();

    let cookie_string = headers
        .get_all("set-cookie")
        .iter()
        .map(|el| {
            let cookie = Cookie::parse(el.to_str().unwrap()).unwrap();
            let (el, el1) = cookie.name_value();
            format!("{el}={el1}")
        })
        .reduce(|el, el1| format!("{el}; {el1}"))
        .unwrap_or_default();

    let url = format!(
        "https://nessa.webuntis.com/WebUntis/api/homeworks/lessons?startDate={}&endDate={}",
        start_date, end_date
    );
    let res = client
        .get(url)
        .header("cookie", cookie_string)
        .header("accept", "application/json")
        .send()
        .unwrap();

    let data: Root = serde_json::from_str(&res.text().unwrap()).unwrap();
    // println!("{:?}", data.data);
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let mut tasks: HashMap<String, Vec<ics::Event>> = HashMap::new();
    data.data.homeworks.into_iter().for_each(|el| {
        let mut task = ics::Event::new(el.id.to_string(), stamp.clone());
        // task.push(Property::new("DTSTART", el.date.to_string()));
        task.push(Property::new("DESCRIPTION", el.text.clone()));
        let lesson = data
            .data
            .lessons
            .iter()
            .find(|le| le.id == el.lesson_id)
            .map(|el| el.subject.clone())
            .unwrap_or_default();
        task.push(Property::new("SUMMARY", format!("🏠 {}", lesson.clone())));
        task.push(Property::new("DTSTART", el.due_date.to_string()));
        task.push(Property::new("DTEND", el.due_date.to_string()));
        match tasks.get_mut(&lesson) {
            Some(vec) => vec.push(task),
            None => {
                tasks.insert(lesson, vec![task]);
            }
        }
    });
    tasks
}
