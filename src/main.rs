use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    thread::{self, sleep},
    time::Duration,
};

use arcshift::ArcShift;
use bytes::Bytes;
use chrono::{Datelike, Days, Local, Timelike};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{
    body::Incoming, header::HeaderValue, server::conn::http1, service::Service, Method, Request,
    Response,
};
use hyper_util::rt::TokioIo;
use ics::{
    properties::{DtEnd, DtStart, Status},
    Event, ICalendar, ToDo,
};
use log::{debug, info};
use simplelog::Config;
use tokio::net::TcpListener;
use untis::{Date, Lesson, Time};

#[derive(Clone)]
struct Svc {
    data: ArcShift<TimeTableData>,
}

impl Svc {
    pub fn new() -> Self {
        Self {
            data: ArcShift::new(TimeTableData::default()),
        }
    }
}

#[derive(Default)]
struct TimeTableData {
    timestamp: i64,
    blocks: HashMap<String, Vec<Event<'static>>>,
    tasks: HashMap<String, Vec<Event<'static>>>,
}

impl Display for TimeTableData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &self
                .blocks
                .keys()
                .cloned()
                .reduce(|acc, x| format!("{acc}\n{x}"))
                .unwrap_or(String::new()),
        )
    }
}

fn fetch_task(mut arc: ArcShift<TimeTableData>) {
    loop {
        match fetch() {
            Ok((timestamp, func)) => {
                // For now won't happen cuz we don't update the timestamp inside the ArcShift. Will need to think about it, as we don't want to not get homework just because no changes have been happening in the timetable.
                if timestamp == arc.timestamp {
                    continue;
                } else {
                    arc.update(func())
                }
            }
            Err(_) => {
                log::error!("Großes Problemchen irgendwie mit Untis zu verbinden. Niiiicht guuuht.")
            }
        }
        sleep(Duration::from_secs(300));
    }
}

type FetchResult = (i64, Box<dyn FnOnce() -> TimeTableData>);

fn fetch() -> Result<FetchResult, untis::Error> {
    let results = untis::schools::search("Gymnasium am markt")?;
    let gamma = results.first().unwrap();

    let mut client = gamma.client_login("Jahrgang12", "Goofy23")?;
    let last_updated = client.last_update_time().unwrap().timestamp();
    let really_fetch = move || {
        info!("Fetching an update");
        let mut data = TimeTableData::default();
        let next_week = Date(
            Date::today()
                .to_chrono()
                .checked_add_days(Days::new(7))
                .unwrap(),
        );
        let timetable = client
            .own_timetable_between(&Date::current_week_begin(), &next_week.relative_week_end())
            .unwrap();

        let mut events: HashMap<String, Vec<Event>> = HashMap::new();
        let subjects = client.subjects().unwrap();
        let mut sorted_timetable: HashMap<(usize, Date), Vec<Lesson>> = HashMap::new();
        timetable.into_iter().for_each(|el| {
            match sorted_timetable.get_mut(&(el.lsnumber, el.date)) {
                Some(v) => v.push(el),
                None => {
                    sorted_timetable.insert((el.lsnumber, el.date), vec![el]);
                }
            }
        });
        sorted_timetable.into_iter().map(|el| el.1).for_each(|el| {
            let double = if el.len() == 2 {
                (el[0].code != el[1].code)
                    || (el[0]
                        .rooms
                        .get(0)
                        .map(|e| e.name.to_string())
                        .unwrap_or_default()
                        != el[1]
                            .rooms
                            .get(0)
                            .map(|e| e.name.to_string())
                            .unwrap_or_default())
            } else {
                false
            };
            let subject = {
                let tmp = el[0]
                    .subjects
                    .first()
                    .map(|el| el.name.to_owned())
                    .unwrap_or_default();
                if tmp == String::default() {
                    ("default").to_owned()
                } else {
                    tmp
                }
            };
            let mut levents = vec![Event::new(
                format!(
                    "{}",
                    el[0].lsnumber
                        + (el[0].date.to_chrono().num_days_from_ce() as usize
                            * el[0].start_time.0.hour() as usize
                            + el[0].start_time.minute() as usize)
                ),
                client
                    .last_update_time()
                    .unwrap()
                    .format("%Y%m%dT%H%M%S")
                    .to_string(),
            )];
            if double {
                let event = Event::new(
                    format!(
                        "{}",
                        el[1].lsnumber
                            + (el[1].date.to_chrono().num_days_from_ce() as usize
                                * el[1].start_time.0.hour() as usize
                                + el[1].start_time.minute() as usize)
                    ),
                    client
                        .last_update_time()
                        .unwrap()
                        .format("%Y%m%dT%H%M%S")
                        .to_string(),
                );
                levents.push(event);
            }

            match subjects.iter().find(|sub| {
                sub.name
                    == el[0]
                        .subjects
                        .first()
                        .map(|el| el.name.clone())
                        .unwrap_or_default()
            }) {
                Some(subj) => {
                    levents.iter_mut().enumerate().for_each(|(idx, ev)| {
                        ev.push(ics::properties::Summary::new(format!(
                            "{} - {}",
                            subj.long_name,
                            el[idx]
                                .rooms
                                .first()
                                .map(|e| e.name.to_string())
                                .unwrap_or_default()
                        )))
                    });
                    levents.iter_mut().enumerate().for_each(|(idx, ev)| {
                        ev.push(ics::properties::Description::new(
                            el[idx]
                                .subjects
                                .first()
                                .map(|el| el.name.clone())
                                .unwrap_or_default(),
                        ))
                    });
                }
                None => {
                    levents.iter_mut().enumerate().for_each(|(idx, ev)| {
                        ev.push(ics::properties::Summary::new(format!(
                            "{}-{}",
                            el[idx]
                                .subjects
                                .first()
                                .map(|el| el.name.clone())
                                .unwrap_or_default(),
                            el[idx]
                                .rooms
                                .first()
                                .map(|el| el.name.clone())
                                .unwrap_or_default()
                        )))
                    });
                }
            };
            if double {
                let (start, end) = start_end_timestamp(&el[0], None);
                levents.get_mut(0).unwrap().push(DtStart::new(start));
                levents.get_mut(0).unwrap().push(DtEnd::new(end));
                match el[0].code {
                    untis::LessonCode::Regular => {
                        if let Some(el) = levents.get_mut(0) {
                            el.push(Status::confirmed())
                        }
                    }
                    untis::LessonCode::Irregular => (),
                    untis::LessonCode::Cancelled => {
                        if let Some(el) = levents.get_mut(0) {
                            el.push(Status::cancelled())
                        }
                    }
                };
                let (start, end) = start_end_timestamp(&el[1], None);
                levents.get_mut(1).unwrap().push(DtStart::new(start));
                levents.get_mut(1).unwrap().push(DtEnd::new(end));
                match el[1].code {
                    untis::LessonCode::Regular => {
                        if let Some(el) = levents.get_mut(1) {
                            el.push(Status::confirmed())
                        }
                    }
                    untis::LessonCode::Irregular => (),
                    untis::LessonCode::Cancelled => {
                        if let Some(el) = levents.get_mut(1) {
                            el.push(Status::cancelled())
                        }
                    }
                };
            } else {
                let (start, end) = start_end_timestamp(&el[0], el.get(1));
                levents.get_mut(0).unwrap().push(DtStart::new(start));
                levents.get_mut(0).unwrap().push(DtEnd::new(end));

                match el[0].code {
                    untis::LessonCode::Regular => levents
                        .iter_mut()
                        .for_each(|ev| ev.push(Status::confirmed())),
                    untis::LessonCode::Irregular => (),
                    untis::LessonCode::Cancelled => levents
                        .iter_mut()
                        .for_each(|ev| ev.push(Status::cancelled())),
                };
            }
            match events.get_mut(&subject) {
                Some(vec) => vec.append(&mut levents),
                None => {
                    events.insert(subject.clone(), levents);
                }
            }
        });

        data.blocks = events;

        let start = Date::current_week_begin().format("%Y%m%d");
        let end = next_week.relative_week_end().format("%Y%m%d");

        let tasks = untis_httpapi::homework(&start.to_string(), &end.to_string());

        data.tasks = tasks;

        data
    };
    Ok((last_updated, Box::new(really_fetch)))
}

// Creates the start and end timestamps of the given Untis lesson
fn start_end_timestamp(lesson: &Lesson, lesson2: Option<&Lesson>) -> (String, String) {
    match lesson2 {
        Some(lesson2) => {
            if lesson.start_time < lesson2.start_time {
                let start = create_timestamp(&lesson.start_time, &lesson.date);
                let end = create_timestamp(&lesson2.end_time, &lesson2.date);
                (start, end)
            } else {
                let start = create_timestamp(&lesson2.start_time, &lesson2.date);
                let end = create_timestamp(&lesson.end_time, &lesson.date);
                (start, end)
            }
        }
        None => {
            let start = create_timestamp(&lesson.start_time, &lesson.date);
            let end = create_timestamp(&lesson.end_time, &lesson.date);
            (start, end)
        }
    }
}

/// Creates an ICS timestamp for the given untis::Time and untis::Date
fn create_timestamp(time: &Time, date: &Date) -> String {
    let date_time = date
        .0
        .and_hms_opt(time.hour(), time.minute(), time.second())
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .to_utc();
    date_time.format("%Y%m%dT%H%M%SZ").to_string()
}

#[tokio::main]
async fn main() {
    simplelog::TermLogger::init(
        log::LevelFilter::Debug,
        Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], 3022));
    let listener = TcpListener::bind(addr).await.unwrap();

    let mut svc = Svc::new();
    let data = svc.data.clone();

    let _fetch_task_handle = thread::spawn(move || fetch_task(data));

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        svc.data.reload();
        let svc = svc.clone();

        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(_err) = http1::Builder::new().serve_connection(io, svc).await {
                log::error!("Error serving");
            }
        });
    }
}

impl Service<Request<Incoming>> for Svc {
    type Response = Response<BoxBody<Bytes, hyper::Error>>;

    type Error = hyper::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        debug!("{req:?}");
        let res = match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => Response::new(full("Ask marvin for help")),
            (&Method::GET, "/ics") => {
                let mut calendar = ICalendar::new("2.0", "ics-rs");
                add_to_calendar(&mut calendar, &self.data, "default");
                req.uri().query().unwrap().split(',').for_each(|el| {
                    add_to_calendar(&mut calendar, &self.data, el);
                });
                let res = Response::new(full(calendar.to_string()));
                let (mut parts, body) = res.into_parts();
                parts
                    .headers
                    .insert("conent-type", HeaderValue::from_static("text/calendar"));
                Response::from_parts(parts, body)
            }
            _ => Response::new(empty()),
        };

        Box::pin(async { Ok(res) })
    }
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

fn add_to_calendar(calendar: &mut ICalendar, data: &ArcShift<TimeTableData>, block_name: &str) {
    if let Some(el) = data.blocks.get(block_name) {
        el.iter().for_each(|el| calendar.add_event(el.clone()))
    }
    if let Some(el) = data.tasks.get(block_name) {
        el.iter().for_each(|el| calendar.add_event(el.clone()))
    }
}
