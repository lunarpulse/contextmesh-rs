use std::time::Duration;

use axum::{
    Json, Router,
    http::{StatusCode, header::LOCATION},
    routing::get,
};
use clap::{Parser, error::ErrorKind};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, process::Command, sync::oneshot};

#[derive(Debug, Parser)]
#[command(name = "oa04-probe", about = "dependency probe")]
struct Args {
    #[arg(long, default_value_t = 7)]
    value: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Reply {
    value: u8,
}

async fn probe_reply() -> Json<Reply> {
    Json(Reply { value: 7 })
}

async fn redirect() -> (StatusCode, [(axum::http::HeaderName, &'static str); 1]) {
    (StatusCode::FOUND, [(LOCATION, "/probe")])
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::try_parse_from(["oa04-probe", "--value", "7"])?;
    assert_eq!(args.value, 7);
    assert_eq!(
        Args::try_parse_from(["oa04-probe", "--help"])
            .expect_err("help must exit through clap")
            .kind(),
        ErrorKind::DisplayHelp
    );

    let expected = blake3::hash(&[9_u8; 32]);
    let matching = blake3::hash(&[9_u8; 32]);
    let different = blake3::hash(&[8_u8; 32]);
    assert_eq!(expected, matching);
    assert_ne!(expected, different);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let app = Router::new()
        .route("/probe", get(probe_reply))
        .route("/redirect", get(redirect));
    let (stop_tx, stop_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = stop_rx.await;
            })
            .await
    });

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(2))
        .http1_only()
        .build()?;
    let response = client
        .get(format!("http://{address}/probe"))
        .send()
        .await?
        .error_for_status()?;
    assert_eq!(response.json::<Reply>().await?, Reply { value: 7 });
    let redirect = client
        .get(format!("http://{address}/redirect"))
        .send()
        .await?;
    assert_eq!(redirect.status(), StatusCode::FOUND);

    let output = Command::new("sh")
        .arg("-c")
        .arg("printf probe")
        .output()
        .await?;
    assert!(output.status.success());
    assert_eq!(output.stdout, b"probe");

    let _ = tokio::time::timeout(Duration::from_millis(1), tokio::signal::ctrl_c()).await;
    let _ = stop_tx.send(());
    server.await??;
    println!("oa04 dependency probe passed");
    Ok(())
}
