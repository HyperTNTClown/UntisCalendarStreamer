mod definitions;
mod fetch;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::File,
    future::Future,
    io::Read,
    net::SocketAddr,
    num::{NonZero, NonZeroU32},
    pin::Pin,
    sync::{Arc, LazyLock},
};

use arcshift::ArcShift;
use bytes::{Buf, Bytes};
use chrono::Local;
use dashmap::DashMap;
use fetch::fetch;
use governor::{DefaultDirectRateLimiter, Quota};
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{
    body::Incoming, header::HeaderValue, server::conn::http1, service::Service, Method, Request,
};
use hyper_util::rt::TokioIo;
use ics::{Event, ICalendar};
use reqwest::{
    cookie::{CookieStore, Jar},
    Client, ClientBuilder, Url,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::{debug, error, info, info_span, warn, warn_span, Instrument, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const GRADES: [isize; 24] = [
    1908, 1905, 1902, 1899, 1896, 1893, 1890, 1887, 1884, 1881, 1878, 1875, 1872, 1869, 1866, 1863,
    1860, 1857, 1854, 1851, 1848, 1845, 1842, 1839,
];

const SCHOOL_SPECIFIC_COOKIES: &str =
    "schoolname=\"_Z3ltbmFzaXVtIGFtIG1hcmt0\"; Tenant-Id=\"5761300\";";

pub static ALIAS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let path = "./alias";
    let mut buf = String::new();
    File::create_new(path).ok();
    File::open(path).unwrap().read_to_string(&mut buf).unwrap();

    buf.split("\n")
        .filter_map(|el| {
            el.find(";")
                .map(|i| el.split_at(i))
                .map(|f| (f.0.to_owned(), f.1.strip_prefix(";").unwrap().to_owned()))
        })
        .collect::<HashMap<String, String>>()
});

#[derive(Clone)]
struct Svc {
    rt: Arc<tokio::runtime::Runtime>,
    client: Client,
    limiter: Arc<DefaultDirectRateLimiter>,
    data: Arc<DashMap<isize, ArcShift<TimeTableData>>>,
}

impl Svc {
    pub fn new(rt: tokio::runtime::Runtime, limiter: DefaultDirectRateLimiter) -> Self {
        Self {
            rt: Arc::new(rt),
            limiter: Arc::new(limiter),
            client: Client::new(),
            data: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, key: isize) -> ArcShift<TimeTableData> {
        match self.data.get(&key) {
            Some(d) => d.clone(),
            None => {
                info!("Generiere neu {}", key);
                let val = ArcShift::new(TimeTableData::default());
                self.data.insert(key, val.clone());
                {
                    let val = val.clone();
                    let span = info_span!("ID", %key);
                    let client = self.client.clone();
                    let limiter = self.limiter.clone();
                    tokio::task::Builder::new()
                        .name(&format!("ID {key}"))
                        .spawn_on(
                            async move { fetch_task(val, client, limiter,  key).instrument(span).await },
                            self.rt.handle(),
                        )
                        .unwrap();
                }
                val
            }
        }
    }
}

#[derive(Default)]
struct TimeTableData {
    blocks: HashMap<String, Vec<Event<'static>>>,
    tasks: HashMap<String, HashSet<Event<'static>>>,
    teachers: HashMap<String, HashSet<String>>,
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

async fn fetch_task(
    mut arc: ArcShift<TimeTableData>,
    client: reqwest::Client,
    limiter: Arc<DefaultDirectRateLimiter>,
    e_id: isize,
) {
    info!("Task für {} gestartet", e_id);
    let mut cookies = String::new();
    'legs: loop {
        if let Some((data, c)) = fetch(e_id, &client, &limiter, cookies.clone()).await {
            cookies = c;
            if data.blocks.is_empty() {
                break 'legs;
            }
            arc.update(data)
        } else {
            error!("Irgendwas ist beim holen der Daten schiefgelaufen, probiere es in 5 Minuten nochmal")
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
    }
    warn!("Task für {} beendet, weil keine Daten bekommen", e_id);
}

#[tokio::main]
async fn main() {
    let registry = tracing_subscriber::registry();

    match tracing_journald::layer() {
        Ok(journald_layer) => {
            registry
                .with(journald_layer)
                // .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
                .init();
            println!("Logging to journald + stderr");
        }
        Err(_) => {
            // Fallback to just stderr/file logging
            registry
                .with(tracing_subscriber::filter::LevelFilter::INFO)
                .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
                .init();
        }
    }

    dotenv::dotenv().ok();
    let addr = SocketAddr::from(([0, 0, 0, 0], 3022));
    let listener = TcpListener::bind(addr).await.unwrap();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let limiter = DefaultDirectRateLimiter::direct(Quota::per_second(NonZero::new(50).unwrap()));

    let svc = Svc::new(rt, limiter);

    // svc.get(-1908);
    for el in GRADES {
        svc.get(-el);
    }

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        svc.clone().get(-1908).reload();

        let svc = svc.clone();
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(_err) = http1::Builder::new().serve_connection(io, svc).await {
                log::error!("Error serving");
            }
        });
    }
}

pub fn create_timestamp(stamp: &str) -> Option<String> {
    let time = chrono::NaiveDateTime::parse_and_remainder(stamp, "%Y-%m-%dT%H:%M")
        .map(|el| el.0)
        .ok()?
        .and_local_timezone(Local)
        .earliest()?
        .to_utc();
    Some(time.format("%Y%m%dT%H%M%SZ").to_string())
}

pub async fn login(
    username: Option<String>,
    password: Option<String>,
    cookies: Option<String>,
) -> Option<(String, String)> {
    if let Some(c) = cookies {
        if let Some((token, cookies)) = try_refresh(c).await {
            info!("Session could be recovered");
            return Some((token, cookies));
        }
    }
    info!("Creating new session and loggin in through oauth");
    let mut untis_cookies = String::from(SCHOOL_SPECIFIC_COOKIES);

    let url = "https://nessa.webuntis.com/WebUntis/oidc/login";
    let cookie_jar = Arc::new(Jar::default());
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .cookie_provider(cookie_jar.clone())
        .build()
        .ok()?;
    let res = client
        .get(url)
        .header("Cookie", &untis_cookies)
        .send()
        .await
        .ok()?;
    let redirect_url = res.url().clone();
    let res = client.get(redirect_url).send().await.ok()?;
    let login_url = res.url().clone();
    let mut params = HashMap::new();
    let username = match username {
        Some(u) => u,
        None => std::env::vars().find(|(k, _)| k == "USERNAME")?.1,
    };
    params.insert("_username", username);
    let password = match password {
        Some(p) => p,
        None => std::env::vars().find(|(k, _)| k == "PASSWORD")?.1,
    };
    params.insert("_password", password);
    let res = client.post(login_url).form(&params).send().await.ok()?;
    let text = res.text().await.ok()?;
    let redirect = text.split(";url=").nth(1)?.split("\">").next()?;
    let res = client.get(redirect).send().await.ok()?;
    let params = construct_oauth_params(
        res.url().to_string(),
        res.text().await.unwrap_or_default().to_string(),
    );
    let _res = client
        .post("https://gamma-achim.de/iserv/oauth/v2/auth")
        .form(&params)
        .send()
        .await
        .ok()?;

    let res = client
        .get("https://nessa.webuntis.com/WebUntis/api/token/new")
        .send()
        .await
        .ok()?;

    let token = res.text().await.ok()?;
    let url = Url::parse("https://nessa.webuntis.com/WebUntis").ok()?;
    let needed_cookies = cookie_jar.cookies(&url)?;
    untis_cookies.push_str(needed_cookies.to_str().ok()?);

    Some((token, untis_cookies))
}

async fn try_refresh(cookies: String) -> Option<(String, String)> {
    let client = reqwest::Client::builder().build().ok()?;
    let res = client
        .get("https://nessa.webuntis.com/WebUntis/api/token/new")
        .header("Cookie", &cookies)
        .send()
        .await
        .ok()?;
    let token = res.text().await.ok()?;
    if token.starts_with("<!doctype html>") {
        warn!("Did not get a token");
        return None;
    }
    Some((token, cookies))
}

fn construct_oauth_params(url: String, text: String) -> HashMap<&'static str, &'static str> {
    let mut params = HashMap::new();
    params.insert("accepted", "");
    params.insert(
        "iserv_oauth_server_authorize_form[client_id]",
        "15_61zgj5ci0q4ows8swo80so0g4wkckgwsg40owkg4k8cc8cg04k",
    );
    params.insert("iserv_oauth_server_authorize_form[response_type]", "code");
    // TODO: parse the URL, as it seems that it is prone to change
    params.insert(
        "iserv_oauth_server_authorize_form[redirect_uri]",
        "https://oidc.webuntis.com/WebUntis/oidc/callback",
    );
    // TODO: decode URI Parts, as it might cause more problems in the future
    let state = url
        .split("state=")
        .nth(1)
        .unwrap()
        .split("&")
        .next()
        .unwrap()
        .replace("%3D", "=");
    params.insert("iserv_oauth_server_authorize_form[state]", state.leak());
    params.insert(
        "iserv_oauth_server_authorize_form[scope]",
        "openid email iserv:webuntis",
    );
    let nonce = url
        .leak()
        .split("nonce=")
        .nth(1)
        .unwrap()
        .split("&")
        .next()
        .unwrap();
    params.insert("iserv_oauth_server_authorize_form[nonce]", nonce);
    let token = {
        text.split("iserv_oauth_server_authorize_form__token")
            .nth(1)
            .unwrap()
            .split("value=\"")
            .nth(1)
            .unwrap()
            .split("\"")
            .next()
            .unwrap()
            .to_owned()
    };
    params.insert("iserv_oauth_server_authorize_form[_token]", token.leak());
    params
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginData {
    username: String,
    password: String,
}

impl Service<Request<Incoming>> for Svc {
    type Response = hyper::http::response::Response<BoxBody<Bytes, hyper::Error>>;

    type Error = hyper::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        debug!("{req:?}");
        let res = match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => {
                let options = self
                    .get(-1908)
                    .blocks
                    .keys()
                    .fold(String::new(), |acc, el| format!("{acc}\n{el}"))
                    .trim()
                    .to_owned();
                hyper::http::response::Response::new(full(options))
            }
            (&Method::GET, "/ics") => {
                let mut calendar = ICalendar::new("2.0", "ics-rs");
                add_to_calendar(&mut calendar, &self.get(-1908), "default");
                req.uri()
                    .query()
                    .unwrap_or_default()
                    .split(',')
                    .for_each(|el| {
                        add_to_calendar(&mut calendar, &self.get(-1908), el);
                    });
                let cal_string = calendar.to_string().replace(",", "\\,").replace(";", "\\;");
                let res = hyper::http::response::Response::new(full(cal_string));
                let (mut parts, body) = res.into_parts();
                parts
                    .headers
                    .insert("conent-type", HeaderValue::from_static("text/calendar"));
                hyper::http::response::Response::from_parts(parts, body)
            }
            (&Method::GET, "/t") => {
                let mut calendar = ICalendar::new("2.0", "ics-rs");
                let teacher = req.uri().query().unwrap_or_default().to_string();
                for g in GRADES {
                    let ttd = self.get(-g);
                    let class = ttd.teachers.get(&teacher);
                    if let Some(c) = class {
                        for c in c {
                            add_to_calendar(&mut calendar, &ttd, c)
                        }
                    }
                }
                let cal_string = calendar.to_string().replace(",", "\\,").replace(";", "\\;");
                let res = hyper::http::response::Response::new(full(cal_string));
                let (mut parts, body) = res.into_parts();
                parts
                    .headers
                    .insert("conent-type", HeaderValue::from_static("text/calendar"));
                hyper::http::response::Response::from_parts(parts, body)
            }
            (&Method::GET, _) => {
                if req.uri().path().starts_with("/ics/") {
                    let id = req
                        .uri()
                        .path()
                        .trim_start_matches("/ics/")
                        .parse::<isize>()
                        .unwrap_or_default();
                    debug!("{id}");
                    // TODO: Id by grade or by person, both are equally possible, just need to differentiate. But prob person to just give out the correct timetable
                    // Query Params for further filtering? If not, just give out everything associated with the id. Possibly also blacklist query params, with exclamation marks or underscores
                    // If by person, just fetch one full week to find the courses they have and then use the grade data (cache person id relations)
                    let mut calendar = ICalendar::new("2.0", "ics-rs");
                    // let mut q = req.uri().query().unwrap_or_default().split(',');
                    self.get(id)
                        .blocks
                        .iter()
                        .filter(|(name, list)| {
                            name.contains("default")
                                || !list.iter().all(|el| el.to_string().contains("➕"))
                        })
                        .for_each(|(k, _)| add_to_calendar(&mut calendar, &self.get(id), k));
                    // add_to_calendar(&mut calendar, &self.get(id), "default");
                    let cal_string = calendar.to_string().replace(",", "\\,").replace(";", "\\;");
                    let res = hyper::http::response::Response::new(full(cal_string));
                    let (mut parts, body) = res.into_parts();
                    parts
                        .headers
                        .insert("conent-type", HeaderValue::from_static("text/calendar"));
                    hyper::http::response::Response::from_parts(parts, body)
                } else {
                    hyper::http::response::Response::new(empty())
                }
            }
            (&Method::POST, "/id") => {
                // TODO: Do login and get the jwt token to fetch the person and class id
                // println!("{:?}", req.body().collect());
                return Box::pin(async move {
                    let collected = req.into_body().collect().await.unwrap();
                    let d =
                        serde_json::from_slice::<LoginData>(collected.aggregate().chunk()).unwrap();
                    let Some((token, _)) = login(Some(d.username), Some(d.password), None).await
                    else {
                        panic!("")
                    };
                    Ok(hyper::http::response::Response::new(full(
                        token.leak().split(".").nth(1).unwrap(),
                    )))
                });
            }
            _ => hyper::http::response::Response::new(empty()),
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
