use std::{
    sync::Arc,
    net::SocketAddr,
    time::Duration,
    thread,
};

use axum::{
    routing::{get, post},
    extract::State,
    response::Html,
    Router,
    Form
};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use hyprtxt::hyprtxt;
use tokio::sync::Mutex;

type SharedState = Arc<Mutex<AppState>>;

struct AppState {
    hosts: Vec<String>,
    job_name: String,
    running: bool,
    batches: usize,
    batch_size: usize,
    completed: usize,
    keyboards: Vec<Keyboard>
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            hosts: vec![],
            job_name: "".into(),
            running: false,
            batches: 0,
            batch_size: 0,
            completed: 0,
            keyboards: vec![]
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
enum UpdateResp {
    Init,
    InProgress {
        batch_size: usize,
        completed: usize,
    },
    BatchComplete {
        keyboards: Vec<Keyboard>,
    },
}

#[derive(Serialize)]
struct BatchReq {
    job_name: String,
    device_name: String,
    batch_size: usize,
    batch_number: usize,
}

#[derive(Deserialize, Clone)]
struct Keyboard {
    score: f32,
    #[serde(with = "BigArray")]
    keys: [Key; 47],
}

#[allow(dead_code)]
#[derive(Deserialize, Clone, Copy)]
struct Key {
    lower: char,
    upper: char,
}

#[derive(Deserialize)]
struct AddServerReq {
    host: String
}

#[derive(Deserialize)]
struct StartJobReq {
    job_name: String,
    batch_size: usize,
    batches: usize,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let state = Arc::new(Mutex::new(AppState::default()));

    let router = Router::new()
        .route("/", get(root))
        .route("/update", get(update))
        .route("/add-server", post(add_server))
        .route("/start-job", post(start_job))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap()
}

async fn root(State(_shared_state): State<SharedState>) -> Html<String> {
    let head = hyprtxt!(
        "head" {
            "meta"* { "charset"="UTF-8" }
            "meta"* { 
                "http-equiv"="X-UA-Compatible"
                "content"="IE-edge"
            }
            "meta"* {
                "name"="viewport"
                "content"="width=device-width, initial-scale=1.0"
            }
            "title" { $: "keyboard generation nonsense" }
            "script" {
                "src"="https://unpkg.com/htmx.org@1.9.2"
                "integrity"="sha384-L6OqL9pRWyyFU3+/bjdSri+iIphTN/bvYyM37tICVyOJkWZLpP2vGn6VUEXgzg6h"
                "crossorigin"="anonymous"
            }
        }
    );
    let body = hyprtxt!(
        "body" {
            "h1" { $: "Keyboard Generation" }
            "h3" { $: "Workers" }
            
            "form" {
                "hx-post"="/add-server"
                "hx-target"="#status"

                "div" {
                    "label" {
                        "for"="host"
                        $: "Host"
                    }
                    "input" {
                        "type"="text"
                        "name"="host"
                        "id"="host"
                    }
                }
                "br"* {}
                "button" {
                    "type"="submit"
                    $: "Add Worker"
                }
            }
            
            "h3" { $: "Start Job" }
            
            "form" {
                "hx-post"="/start-job"
                "hx-swap"="outerHTML"

                "div" {
                    "label" {
                        "for"="job_name"
                        $: "Job Name"
                    }
                    "input" {
                        "type"="text"
                        "name"="job_name"
                        "id"="job_name"
                    }
                }
                "div" {
                    "label" {
                        "for"="batch_size"
                        $: "Batch Size"
                    }
                    "input" {
                        "type"="number"
                        "name"="batch_size"
                        "id"="batch_size"
                    }
                }
                "div" {
                    "label" {
                        "for"="batches"
                        $: "Batches"
                    }
                    "input" {
                        "type"="number"
                        "name"="batches"
                        "id"="batches"
                    }
                }
                "br"* {}
                "button" {
                    "type"="submit"
                    $: "Start Job"
                }
            }

            "div" {
                "hx-get"="/update"
                "hx-trigger"="every 5s"
                "id"="status"
                "h3" { $: "Status: INIT" }
            }
            "br"* {}
            "br"* {}
            "img" {
                "src"="https://htmx.org/img/createdwith.jpeg" 
                "alt"="hypermedia is my passion"
                "height"="90"
            }
        }
    );
    Html(vec!["<!DOCTYPE html>".to_string(), head, body].join(""))
}

async fn update(State(shared_state): State<SharedState>) -> Html<String> {
    let state = shared_state.lock().await;

    let update_layout = |inner: String| -> Html<String> {
        Html(hyprtxt!(
            "div" {
                $: inner
            }
        ))
    };
    
    if state.running {
        update_layout(hyprtxt!(
            "div" {
                "h3" {
                    $: "Job "
                    $: state.job_name
                    $: " Running"
                }
                $: keyboard(state.keyboards.clone())
            }
        ))
    } else if state.keyboards.len() > 0 {
        update_layout(hyprtxt!(
            "div" {
                "h3" {
                    $: "Job "
                    $: state.job_name
                    $: " Complete"
                }
                $: keyboard(state.keyboards.clone())
            }
        ))
    } else {
        update_layout(hyprtxt!(
            "h3" { $: "INIT" }
        ))
    }
}

async fn add_server(
    State(shared_state): State<SharedState>,
    Form(add_server_req): Form<AddServerReq>,
) -> Html<String> {
    let mut state = shared_state.lock().await;
    state.hosts.push(add_server_req.host);

    Html(hyprtxt!(
        "div" {
            "h3" { $: "Current Servers" }
            "ul" {
                $: state.hosts
                    .iter()
                    .map(|s| hyprtxt!("li" { $: s }))
                    .collect::<Vec<String>>()
                    .concat()
            }
        }
    ))
}

#[axum::debug_handler]
async fn start_job(
    State(shared_state): State<SharedState>,
    Form(StartJobReq { job_name, batch_size, batches }): Form<StartJobReq>,
) -> Html<String> {
    let mut state = shared_state.lock().await;
    if state.running {
        return Html(hyprtxt!("h1" {
            "style"="color: red;"
            $: "ERROR: job already in progress"
        }))
    }
    state.job_name = job_name;
    state.batches = batches;
    state.batch_size = batch_size;
    state.running = true;
    let hosts = state.hosts.clone();

    for host in hosts {
        let thread_state = shared_state.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            loop {
                let resp = client
                    .get(host.to_string() + "/update")
                    .send()
                    .await
                    .expect("failed request")
                    .json()
                    .await
                    .expect("failed parse");
                
                match resp {
                    UpdateResp::InProgress { .. } => thread::sleep(Duration::new(5, 0)),
                    UpdateResp::Init => {
                        let mut state = thread_state.lock().await;
                        if state.completed < state.batches {
                            client
                                .post(host.to_string() + "/new")
                                .json(&BatchReq {
                                    job_name: state.job_name.clone(),
                                    device_name: host.to_string(),
                                    batch_size: state.batch_size,
                                    batch_number: state.completed,
                                })
                                .send()
                                .await
                                .expect("failed post");
                        } else {
                            state.running = false;
                            break;
                        }
                    },
                    UpdateResp::BatchComplete { keyboards } => {
                        let mut state = thread_state.lock().await;
                        state.completed += 1;
                        state.keyboards.extend(keyboards.into_iter());
                        if state.completed < state.batches {
                            client
                                .post(host.to_string() + "/new")
                                .json(&BatchReq {
                                    job_name: state.job_name.clone(),
                                    device_name: host.to_string(),
                                    batch_size: state.batch_size,
                                    batch_number: state.completed,
                                })
                                .send()
                                .await
                                .unwrap();
                        } else {
                            state.running = false;
                            break;
                        }
                    },
                }
            }
        });
    }

    Html(hyprtxt!(
        "div" {
            "h3" {
                $: "Job "
                $: state.job_name
                $: " Started"
            }
            "button" {
                $: "update"
            }
            "button" {
                $: "stop job"
            }
        }
    ))
}

fn keyboard(mut keyboards: Vec<Keyboard>) -> String {
    if keyboards.len() == 0 { return "".to_string()}
    keyboards.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
    let keyboard = &keyboards[0];
    hyprtxt!(
        "div" {
            "class"="keyboard"
            "div" {
                "class"="row"
                $: &keyboard.keys[0..13]
                    .iter()
                    .map(|key| hyprtxt!("button" {
                        "class"="button"
                        $: key.upper
                    }))
                    .collect::<Vec<String>>()
                    .concat()
            }
            "div" {
                "class"="row"
                $: &keyboard.keys[13..26]
                    .iter()
                    .map(|key| hyprtxt!("button" {
                        "class"="button"
                        $: key.upper
                    }))
                    .collect::<Vec<String>>()
                    .concat()
            }
            "div" {
                "class"="row"
                $: &keyboard.keys[26..37]
                    .iter()
                    .map(|key| hyprtxt!("button" {
                        "class"="button"
                        $: key.upper
                    }))
                    .collect::<Vec<String>>()
                    .concat()
            }
            "div" {
                "class"="row"
                $: &keyboard.keys[37..47]
                    .iter()
                    .map(|key| hyprtxt!("button" {
                        "class"="button"
                        $: key.upper
                    }))
                    .collect::<Vec<String>>()
                    .concat()
            }
        }
    )
}
