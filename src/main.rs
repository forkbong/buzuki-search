use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::service::{make_service_fn, service_fn};
use hyper::Server;
use hyper::{header, Body, Method, Request, Response, StatusCode};
use log::{error, info, warn};
use url::form_urlencoded;

mod greek_lower_caser;
mod search_engine;
mod song;
mod tokenizer;
mod utils;

use crate::search_engine::SearchEngine;

async fn buzuki(
    request: Request<Body>,
    search_engine: SearchEngine,
) -> Result<Response<Body>, hyper::Error> {
    fn get_json_response(status: StatusCode, body: Body) -> Result<Response<Body>, hyper::Error> {
        Ok(Response::builder()
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .status(status)
            .body(body)
            .unwrap())
    }

    fn search(
        request: &Request<Body>,
        search_engine: &SearchEngine,
        simple: bool,
    ) -> (StatusCode, String) {
        let mut response = String::from("[]");
        let mut status = StatusCode::NOT_FOUND;
        if let Some(query) = request.uri().query() {
            let query_map = form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect::<HashMap<String, String>>();
            if let Some(value) = query_map.get("q") {
                let results = search_engine.search(value, simple);
                match results {
                    Ok(string) => {
                        response = string;
                        status = StatusCode::OK;
                    }
                    Err(e) => {
                        warn!("error: {}\nquery: {}", e, query);
                        response = format!("{{\"error\": \"{}\"}}", e);
                        status = StatusCode::INTERNAL_SERVER_ERROR;
                    }
                }
            }
        }
        (status, response)
    }

    match (request.method(), request.uri().path()) {
        (&Method::GET, "/") => {
            let (status, response) = search(&request, &search_engine, true);
            get_json_response(status, Body::from(response))
        }
        (&Method::GET, "/autocomplete/") => {
            let (status, response) = search(&request, &search_engine, false);
            get_json_response(status, Body::from(response))
        }
        _ => get_json_response(StatusCode::NOT_FOUND, Body::from("[]")),
    }
}

/// Initialize env_logger to use info level by default.
fn init_logger() {
    let env = env_logger::Env::default().default_filter_or("info");
    let mut builder = env_logger::Builder::from_env(env);
    builder.init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_logger();

    let key = "BUZUKI_SONGDIR";
    let songdir = match std::env::var(key) {
        Ok(val) => val,
        Err(e) => {
            error!("Couldn't get {}: {}", key, e);
            std::process::exit(1);
        }
    };

    let search_engine = SearchEngine::new(&songdir)?;

    let addr = SocketAddr::from(([127, 0, 0, 1], 1337));

    let make_service = make_service_fn(move |_| {
        let search_engine = search_engine.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |request| {
                buzuki(request, search_engine.clone())
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    info!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
