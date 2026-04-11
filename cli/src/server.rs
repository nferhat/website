//! A quick and dirty HTTP file server used for a dev mode.
//!
//! It was put up using the following example from Axum:
//! <https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs>

use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::header::CACHE_CONTROL;
use axum::response::sse::Event as SseEvent;
use axum::response::{IntoResponse, Sse};
use axum::{Router, http};
use futures::StreamExt;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::sync::{broadcast, mpsc};
use tokio::{fs, runtime};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::compiler::{compile_file, compile_styles};
use crate::config::Config;

pub async fn run(
    root: PathBuf,
    build_dir: PathBuf,
    config: Config,
    port: u16,
) -> anyhow::Result<()> {
    let config = Arc::new(config);
    let (tx, _) = broadcast::channel::<()>(32);
    // watch_directory(&root, tx).await?;
    _ = tokio::join!(
        serve(serve_build(&build_dir, tx.clone()), port),
        watch_for_changes(&root, &build_dir, Arc::clone(&config), tx)
    );
    Ok(())
}

fn serve_build(path: &Path, tx: broadcast::Sender<()>) -> Router {
    // Disable caching in the browser.
    let disable_caching_layer = SetResponseHeaderLayer::overriding(
        CACHE_CONTROL,
        http::HeaderValue::from_static("no-cache"),
    );

    Router::new()
        // Hot reloading route. We use an SSE event with some additional javascript on the client to
        // achieve this. We also include the script needed to reload.
        .route("/__reload__", axum::routing::get(reload_sse))
        .route("/reload-script.js", axum::routing::get(reload_script))
        .with_state(tx)
        .layer(disable_caching_layer)
        .fallback_service(ServeDir::new(path))
}

async fn reload_script() -> impl IntoResponse {
    (
        [("Content-Type", "application/javascript")],
        include_str!("../res/reload-script.js"),
    )
}

async fn reload_sse(
    State(tx): State<broadcast::Sender<()>>,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = tx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(_) => Some(Ok(SseEvent::default().data("reload"))),
            Err(_) => None,
        }
    });

    Sse::new(stream)
}

fn async_watcher() -> notify::Result<(
    RecommendedWatcher,
    mpsc::Receiver<notify::Result<notify::Event>>,
)> {
    let (tx, rx) = mpsc::channel(1);
    let handle = runtime::Handle::current();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(
        move |res| {
            let tx = tx.clone();
            handle.block_on(async move {
                if tx.send(res).await.is_err() {
                    warn!("Failed to send notify update to channel")
                }
            });
        },
        Default::default(),
    )?;

    Ok((watcher, rx))
}

async fn watch_for_changes(
    root: &Path,
    build_dir: &Path,
    config: Arc<Config>,
    reload_sender: broadcast::Sender<()>,
) -> notify::Result<()> {
    // We gotta make it absolute for things to work here.
    // Notably, we use Path::strip_prefix in order to correctly calculate the build directories
    let root = if root.is_absolute() {
        root.to_owned()
    } else {
        env::current_dir().expect("no cwd").join(root)
    };

    let (mut watcher, mut rx) = async_watcher()?;
    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(root.as_ref(), RecursiveMode::Recursive)?;

    while let Some(res) = rx.recv().await {
        let Ok(event) = res else { continue };
        if !matches!(event.kind, EventKind::Modify(_)) {
            continue;
        }

        for path in event.paths {
            if is_style(&path) {
                debug!(?path, "Triggering stylesheets rebuild due to path change");
                let style_input = root.join(&config.styling.root_file);
                let contents = match compile_styles(&style_input, &config.styling.load_paths).await
                {
                    Ok(contents) => contents,
                    Err(err) => {
                        error!(?err, "Failed to rebuild stylesheets");
                        continue;
                    }
                };

                let output_path = build_dir.join("style.css");
                match fs::write(&output_path, contents).await {
                    Ok(()) => (),
                    Err(err) => {
                        error!(?err, "Failed to write new stylesheet contents");
                        continue;
                    }
                }

                reload_sender.send(()).ok();
                info!("Rebuilt stylesheets")
            } else if is_markdown(&path) {
                debug!(?path, "Triggering page rebuild due to path change");
                let path_relative = path.strip_prefix(&root).expect("relative to root");
                let output_path = build_dir.join(path_relative).with_extension("html");

                let contents = match compile_file(&path, &output_path).await {
                    Ok(contents) => contents,
                    Err(err) => {
                        error!(?err, ?path_relative, "Failed to rebuild page");
                        continue;
                    }
                };

                match fs::write(&output_path, contents).await {
                    Ok(()) => (),
                    Err(err) => {
                        error!(?err, "Failed to write new stylesheet contents");
                        continue;
                    }
                }

                reload_sender.send(()).ok();
                info!(path = ?output_path, "Rebuilt page");
            }
        }
    }

    Ok(())
}

fn is_style(p: &Path) -> bool {
    let Some(ext) = p.extension() else {
        return false;
    };

    ext == "sass" || ext == "scss"
}

fn is_markdown(p: &Path) -> bool {
    let Some(ext) = p.extension() else {
        return false;
    };

    ext == "md" || ext == "markdown" || ext == "mdown"
}

async fn serve(app: Router, port: u16) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(TraceLayer::new_for_http())).await?;
    Ok(())
}
