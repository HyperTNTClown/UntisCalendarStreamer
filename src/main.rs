use std::{
    collections::HashMap,
    fmt::Display,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    thread::{self, sleep},
    time::Duration,
};

use arcshift::ArcShift;
use bytes::Bytes;
use chrono::{Datelike, Days, Timelike};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{body::Incoming, server::conn::http1, service::Service, Method, Request, Response};
use hyper_util::rt::TokioIo;
use ics::{
    properties::{DtEnd, DtStart},
    Event, ICalendar,
};
use tokio::net::TcpListener;
use untis::{Date, IdItem};

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
    // let mut interval = interval(Duration::from_secs(300));

    loop {
        match fetch() {
            Ok((timestamp, func)) => {
                if timestamp == arc.timestamp {
                    continue;
                } else {
                    arc.update(func())
                }
            }
            Err(_) => {
                eprintln!("Großes Problemchen irgendwie mit Untis zu verbinden. Niiiicht guuuht.")
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
        timetable.into_iter().for_each(|el| {
            if let Some(IdItem { name: subject, .. }) = el.subjects.first() {
                let mut event = Event::new(
                    format!(
                        "{}",
                        el.lsnumber
                            + (el.date.to_chrono().num_days_from_ce() as usize
                                * el.start_time.0.hour() as usize
                                + el.start_time.minute() as usize)
                    ),
                    client
                        .last_update_time()
                        .unwrap()
                        .format("%Y%m%dT%H%M%S")
                        .to_string(),
                );
                let start = el
                    .date
                    .0
                    .and_hms_opt(
                        el.start_time.hour(),
                        el.start_time.minute(),
                        el.start_time.second(),
                    )
                    .unwrap()
                    .format("%Y%m%dT%H%M%S")
                    .to_string();
                let end = el
                    .date
                    .0
                    .and_hms_opt(
                        el.end_time.hour(),
                        el.end_time.minute(),
                        el.end_time.second(),
                    )
                    .unwrap()
                    .format("%Y%m%dT%H%M%S")
                    .to_string();
                event.push(ics::properties::Summary::new(format!(
                    "{}-{}",
                    el.subjects.first().unwrap().name,
                    el.rooms.first().unwrap().name
                )));
                event.push(DtStart::new(start));
                event.push(DtEnd::new(end));
                match events.get_mut(subject) {
                    Some(vec) => vec.push(event),
                    None => {
                        events.insert(subject.clone(), vec![event]);
                    }
                }
            }
        });

        data.blocks = events;

        // println!("{:?}", events.keys());

        data
    };
    Ok((last_updated, Box::new(really_fetch)))
}

// fn main() {
//     fetch().unwrap().1();
// }

#[tokio::main]
async fn main() {
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
                eprintln!("Error serving");
            }
        });
    }
}

impl Service<Request<Incoming>> for Svc {
    type Response = Response<BoxBody<Bytes, hyper::Error>>;

    type Error = hyper::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let res = match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => Response::new(full(req.uri().query().unwrap().to_string())),
            (&Method::GET, "/ics") => {
                let mut calender = ICalendar::new("2.0", "ics-rs");
                req.uri().query().unwrap().split(',').for_each(|el| {
                    self.data
                        .blocks
                        .get(el)
                        .unwrap()
                        .iter()
                        .for_each(|el| calender.add_event(el.clone()));
                });
                Response::new(full(calender.to_string()))
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
