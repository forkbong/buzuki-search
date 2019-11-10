use std::collections::HashMap;

use futures::{future, Future};
use hyper::service::service_fn;
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use log::{error, info, warn};
use url::form_urlencoded;

mod greek_lower_caser;
mod search_engine;
mod song;
mod tokenizer;
mod utils;

use crate::search_engine::SearchEngine;

fn service(
    request: &Request<Body>,
    search_engine: &SearchEngine,
) -> Box<dyn Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    fn get_json_response(
        status: StatusCode,
        body: Body,
    ) -> Box<dyn Future<Item = Response<Body>, Error = hyper::Error> + Send> {
        Box::new(future::ok(
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .status(status)
                .body(body)
                .unwrap(),
        ))
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

fn serve(search_engine: SearchEngine) {
    let addr = "127.0.0.1:1337".parse().unwrap();

    hyper::rt::run(future::lazy(move || {
        let new_service = move || {
            let search_engine = search_engine.clone();
            service_fn(move |request| service(&request, &search_engine))
        };

        let server = match Server::try_bind(&addr) {
            Ok(builder) => builder
                .serve(new_service)
                .map_err(|e| error!("server error: {}", e)),
            Err(e) => {
                error!("Couldn't bind to {}: {}", addr, e);
                std::process::exit(1);
            }
        };

        info!("Listening on http://{}", addr);

        server
    }));
}

/// Initialize env_logger to use info level by default.
fn init_logger() {
    let env = env_logger::Env::default().default_filter_or("info");
    let mut builder = env_logger::Builder::from_env(env);
    builder.init();
}

fn main() -> tantivy::Result<()> {
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

    serve(search_engine);

    Ok(())
}
