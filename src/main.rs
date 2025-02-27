mod definitions;
mod fetch;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs::File,
    future::Future,
    io::Read,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, LazyLock},
    thread::sleep,
};

use arcshift::ArcShift;
use bytes::Bytes;
use chrono::Local;
use fetch::fetch;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{
    body::Incoming, header::HeaderValue, server::conn::http1, service::Service, Method, Request,
};
use hyper_util::rt::TokioIo;
use ics::{Event, ICalendar};
use log::debug;
use reqwest::{
    blocking::Response,
    cookie::{CookieStore, Jar},
    Url,
};
use tokio::net::TcpListener;

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
    blocks: HashMap<String, Vec<Event<'static>>>,
    tasks: HashMap<String, HashSet<Event<'static>>>,
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

fn fetch_task(mut arc: ArcShift<TimeTableData>) -> ! {
    loop {
        arc.update(fetch());
        sleep(std::time::Duration::from_secs(300));
    }
}

// fn main() {
//     dotenv::dotenv().ok();
//     fetch::fetch();
// }

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let addr = SocketAddr::from(([0, 0, 0, 0], 3022));
    let listener = TcpListener::bind(addr).await.unwrap();

    let mut svc = Svc::new();
    let data = svc.data.clone();

    let _fetch_task_handle = std::thread::spawn(move || fetch_task(data));

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

pub fn create_timestamp(stamp: &str) -> String {
    let time = chrono::NaiveDateTime::parse_and_remainder(stamp, "%Y-%m-%dT%H:%M")
        .map(|el| el.0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .to_utc();
    time.format("%Y%m%dT%H%M%SZ").to_string()
}

pub fn login() -> (String, String) {
    let mut untis_cookies = String::from(SCHOOL_SPECIFIC_COOKIES);

    let url = "https://nessa.webuntis.com/WebUntis/oidc/login";
    let cookie_jar = Arc::new(Jar::default());
    let client = reqwest::blocking::Client::builder()
        .cookie_store(true)
        .cookie_provider(cookie_jar.clone())
        .build()
        .unwrap();
    let res = client
        .get(url)
        .header("Cookie", &untis_cookies)
        .send()
        .unwrap();
    let redirect_url = res.url().clone();
    let res = client.get(redirect_url).send().unwrap();
    let login_url = res.url().clone();
    let mut params = HashMap::new();
    let (_, username) = std::env::vars().find(|(k, _)| k == "USERNAME").unwrap();
    params.insert("_username", username);
    let (_, password) = std::env::vars().find(|(k, _)| k == "PASSWORD").unwrap();
    params.insert("_password", password);
    let res = client.post(login_url).form(&params).send().unwrap();
    let text = res.text().unwrap();
    let redirect = text
        .split(";url=")
        .nth(1)
        .unwrap()
        .split("\">")
        .next()
        .unwrap();
    let res = client.get(redirect).send().unwrap();
    let params = construct_oauth_params(res);
    let res = client
        .post("https://gamma-achim.de/iserv/oauth/v2/auth")
        .form(&params)
        .send()
        .unwrap();

    println!("{res:?}");

    let res = client
        .get("https://nessa.webuntis.com/WebUntis/api/token/new")
        .send()
        .unwrap();

    let token = res.text().unwrap();
    println!("{token}");
    let url = Url::parse("https://nessa.webuntis.com/WebUntis").unwrap();
    let needed_cookies = cookie_jar.cookies(&url).unwrap();
    println!("{needed_cookies:?}");
    untis_cookies.push_str(needed_cookies.to_str().unwrap());

    (token, untis_cookies)
}

fn construct_oauth_params(res: Response) -> HashMap<&'static str, &'static str> {
    let url = res.url().to_string().leak();
    let mut params = HashMap::new();
    params.insert("accepted", "");
    params.insert(
        "iserv_oauth_server_authorize_form[client_id]",
        "15_61zgj5ci0q4ows8swo80so0g4wkckgwsg40owkg4k8cc8cg04k",
    );
    params.insert("iserv_oauth_server_authorize_form[response_type]", "code");
    params.insert(
        "iserv_oauth_server_authorize_form[redirect_uri]",
        "https://nessa.webuntis.com/WebUntis/oidc/callback",
    );
    let state = url
        .split("state=")
        .nth(1)
        .unwrap()
        .split("&")
        .next()
        .unwrap();
    params.insert("iserv_oauth_server_authorize_form[state]", state);
    params.insert(
        "iserv_oauth_server_authorize_form[scope]",
        "openid email iserv:webuntis",
    );
    let nonce = url
        .split("nonce=")
        .nth(1)
        .unwrap()
        .split("&")
        .next()
        .unwrap();
    params.insert("iserv_oauth_server_authorize_form[nonce]", nonce);
    let token = {
        let text = res.text().unwrap();
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
    println!("{params:?}");
    params
}

impl Service<Request<Incoming>> for Svc {
    type Response = hyper::http::response::Response<BoxBody<Bytes, hyper::Error>>;

    type Error = hyper::Error;

    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        debug!("{req:?}");
        let res = match (req.method(), req.uri().path()) {
            (&Method::GET, "/") => {
                hyper::http::response::Response::new(full("Ask marvin for help"))
            }
            (&Method::GET, "/ics") => {
                let mut calendar = ICalendar::new("2.0", "ics-rs");
                add_to_calendar(&mut calendar, &self.data, "default");
                req.uri().query().unwrap().split(',').for_each(|el| {
                    add_to_calendar(&mut calendar, &self.data, el);
                });
                let res = hyper::http::response::Response::new(full(calendar.to_string()));
                let (mut parts, body) = res.into_parts();
                parts
                    .headers
                    .insert("conent-type", HeaderValue::from_static("text/calendar"));
                hyper::http::response::Response::from_parts(parts, body)
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
