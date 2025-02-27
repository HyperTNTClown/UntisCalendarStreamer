use std::collections::HashMap;

use chrono::{Days, Local, NaiveDate, NaiveTime};
use ics::{
    properties::{Description, DtEnd, DtStart, Summary},
    Event,
};
use reqwest::blocking::{Client, RequestBuilder};

use crate::{
    create_timestamp,
    definitions::{CalendarEntry, Root, Status},
    login, TimeTableData, ALIAS,
};

const NEGATIVE_OFFSET: u64 = 14;
const POSITIVE_OFFSET: usize = 35;

pub fn fetch() -> TimeTableData {
    let (token, cookies) = login();
    let client = Client::new();
    let req_builder = client
        .get("https://nessa.webuntis.com/WebUntis/api/rest/view/v2/calendar-entry/detail")
        .bearer_auth(token)
        .header("Cookie", cookies);

    let starting_day = Local::now()
        .date_naive()
        .week(chrono::Weekday::Mon)
        .first_day()
        - Days::new(NEGATIVE_OFFSET);

    starting_day
        .iter_days()
        .take(NEGATIVE_OFFSET as usize + POSITIVE_OFFSET)
        .map(|day| {
            let req = req_builder.try_clone().unwrap();
            fetch_for_day(day, req)
        })
        .reduce(combine_ttd)
        .unwrap()
}

fn combine_ttd(mut ttd1: TimeTableData, ttd2: TimeTableData) -> TimeTableData {
    for (subj, mut v) in ttd2.blocks {
        match ttd1.blocks.get_mut(&subj) {
            Some(vec) => vec.append(&mut v),
            None => {
                ttd1.blocks.insert(subj, v);
            }
        }
    }
    for (subj, mut v) in ttd2.tasks {
        match ttd1.tasks.get_mut(&subj) {
            Some(vec) => vec.append(&mut v),
            None => {
                ttd1.tasks.insert(subj, v);
            }
        }
    }

    ttd1
}

fn fetch_for_day(day: NaiveDate, req_builder: RequestBuilder) -> TimeTableData {
    let mut ttd = TimeTableData::default();

    let res = req_builder
        .query(&generate_params_for_date(day))
        .send()
        .unwrap();
    println!("{:?}", res);

    let data = res.json::<Root>().unwrap();

    ttd.tasks = data
        .calendar_entries
        .iter()
        .filter_map(|entry| create_hw_events(entry))
        .collect::<HashMap<_, _>>();

    ttd.blocks = HashMap::new();
    data.calendar_entries.into_iter().for_each(|entry| {
        let (subj, ev) = create_block_event(entry);
        match ttd.blocks.get_mut(&subj) {
            Some(vec) => vec.push(ev),
            None => {
                ttd.blocks.insert(subj, vec![ev]);
            }
        }
    });

    ttd
}

fn create_hw_events(entry: &CalendarEntry) -> Option<(String, Vec<Event<'static>>)> {
    if entry.homeworks.is_empty() {
        return None;
    };
    let subject = entry.subject.display_name.clone();
    let hw = entry
        .homeworks
        .iter()
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

    Some((subject, hw))
}

fn create_block_event(entry: CalendarEntry) -> (String, Event<'static>) {
    let id = entry.id.to_string();
    let dtstamp = chrono::Local::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut ev = Event::new(id, dtstamp);

    let status = match entry.status {
        Status::Cancelled => ics::properties::Status::cancelled(),
        _ => ics::properties::Status::confirmed(),
    };
    ev.push(status);
    ev.push(generate_summary(entry.clone()));
    add_timestamps(&mut ev, &entry);
    (entry.subject.display_name, ev.clone())
}

fn generate_summary(entry: CalendarEntry) -> ics::properties::Summary<'static> {
    let name = ALIAS
        .get_key_value(&entry.subject.display_name)
        .map(|(_, val)| val.clone())
        .unwrap_or(entry.subject.long_name);
    let room = entry
        .rooms
        .into_iter()
        .find(|el| el.status != Status::Removed)
        .unwrap();

    let mut sum = format!("{} - {}", name, room.display_name);
    if room.status == Status::Substitution {
        sum = "🔄 ".to_owned() + &sum;
    }
    if entry.type_field == crate::definitions::Type::AddiotionalPeriod {
        sum = "➕ ".to_owned() + &sum;
    }
    ics::properties::Summary::new(sum)
}

fn add_timestamps(event: &mut Event<'_>, entry: &CalendarEntry) {
    event.push(DtStart::new(create_timestamp(&entry.start_date_time)));
    event.push(DtEnd::new(create_timestamp(&entry.end_date_time)));
}

fn generate_params_for_date(date: NaiveDate) -> HashMap<String, String> {
    let mut map = HashMap::new();

    map.insert("elementId".to_owned(), "1708".to_owned());
    map.insert("elementType".to_owned(), "1".to_owned());

    let start_time = date.and_time(NaiveTime::MIN);
    let start = start_time.to_string().replace(" ", "T");
    map.insert("startDateTime".to_owned(), start);

    let end_time = date.and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap());
    let end = end_time.to_string().replace(" ", "T");
    map.insert("endDateTime".to_owned(), end);

    map
}
