use std::collections::HashMap;

use chrono::{format, Days, NaiveTime, Weekday};
use ics::{
    properties::{Description, DtEnd, DtStart, Summary},
    Event,
};

use crate::{
    create_timestamp,
    hw_definitions::{CalendarEntry, Root},
    login,
};

pub fn fetch_homework() -> HashMap<String, Vec<Event<'static>>> {
    let (token, cookies) = login();

    let client = reqwest::blocking::Client::new();

    let mut map: HashMap<String, Vec<Event<'_>>> = HashMap::new();
    for i in 0..12 {
        let mut q_params: HashMap<&str, &str> = HashMap::new();
        q_params.insert("elementId", "1708");
        q_params.insert("elementType", "1");
        let s_date = chrono::Local::now()
            .date_naive()
            .week(Weekday::Mon)
            .first_day()
            .checked_add_days(Days::new(i))
            .unwrap()
            .and_time(NaiveTime::MIN);
        let start = s_date.to_string().replace(" ", "T");
        q_params.insert("startDateTime", &start);

        let e_date = s_date
            .checked_add_days(Days::new(i))
            .unwrap()
            .date()
            .and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap());
        let end = e_date.to_string().replace(" ", "T");
        q_params.insert("endDateTime", &end);

        let res = client
            .get("https://nessa.webuntis.com/WebUntis/api/rest/view/v2/calendar-entry/detail")
            .query(&q_params)
            .header("Cookie", &cookies)
            .bearer_auth(&token)
            .send()
            .unwrap();

        let data = res.json::<Root>().unwrap();

        // println!("{data:?}");

        let blocks_with_homework = data
            .calendar_entries
            .into_iter()
            .filter(|el| !el.homeworks.is_empty())
            .collect::<Vec<_>>();
        println!("{}", blocks_with_homework.len());

        blocks_with_homework
            .into_iter()
            .map(|el| create_event_string_tuple(el))
            .for_each(|(subject, mut tasks)| match map.get_mut(&subject) {
                Some(vec) => vec.append(&mut tasks),
                None => {
                    map.insert(subject, tasks);
                }
            })
    }

    map
}

fn create_event_string_tuple(entry: CalendarEntry) -> (String, Vec<Event<'static>>) {
    let subject = entry.subject.display_name;
    let hw = entry
        .homeworks
        .into_iter()
        .map(|el| {
            let dtstamp = create_timestamp(&el.date_time);
            let mut task = Event::new(el.id.to_string(), dtstamp);
            let stamp = el.due_date_time.split("T").next().unwrap().replace("-", "");
            task.push(DtStart::new(stamp.clone()));
            task.push(DtEnd::new(stamp.clone()));
            task.push(Summary::new(format!("🏠 {}", subject.clone())));
            task.push(Description::new(el.text.replace("\n", "\\n")));
            task
        })
        .collect::<Vec<_>>();

    (subject, hw)
}
