//! A quick and dirty HTTP file server used for a dev mode.
//!
//! It was put up using the following example from Axum:
//! <https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs>

use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::State;
use axum::response::sse::Event as SseEvent;
use axum::response::{IntoResponse, Sse};
use compiler::{Compiler, Config};
use eyre::Context as _;
use futures::StreamExt;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::runtime;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

pub async fn run(root: PathBuf, config: Config, port: u16) -> eyre::Result<()> {
    let mut compiler = Compiler::new(root.clone().into_boxed_path(), config)
        .await
        .context("failed to init compiler")?;
    // Hot-reloading and whatnot
    compiler.set_dev_mode(true);
    // First do a full-pass compilation
    compiler
        .compile_all()
        .await
        .context("failed to run initial build")?;

    let static_assets_path = root.join("static");

    let (reload_sender, _) = broadcast::channel::<()>(32);
    let build_dir = root.join("dist");
    _ = tokio::join!(
        serve(reload_sender.clone(), &build_dir, &static_assets_path, port),
        watch_for_changes(&root, compiler, reload_sender.clone())
    );

    Ok(())
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
    mut compiler: Compiler,
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
            if is_config(&path) {
                match compiler.reload_configs(&path).await {
                    Ok(()) => {
                        reload_sender.send(()).ok();
                    }
                    Err(err) => {
                        warn!(?err, "Failed to reload website config");
                        continue;
                    }
                }
            } else if is_style(&path) {
                trace!(?path, "Triggering site rebuild due to style change change");
                match compiler.compile_all().await {
                    Ok(()) => {
                        reload_sender.send(()).ok();
                        info!("Rebuilt stylesheets")
                    }
                    Err(err) => {
                        warn!(?err, "Failed to rebuild stylesheets");
                        continue;
                    }
                }
            } else if is_djot(&path) {
                trace!(?path, "Triggering page rebuild due to path change");

                match compiler.recompile_page(&path, true).await {
                    Ok(()) => {
                        reload_sender.send(()).ok();
                        info!(?path, "Rebuilt page")
                    }
                    Err(err) => {
                        warn!(?err, "Failed to rebuild page");
                        continue;
                    }
                }
            } else if path.to_string_lossy().contains("templates") {
                trace!(?path, "Reloading website templates");
                // FIXME: Incremental recompilation for single pages
                match compiler.reload_template(&path).await {
                    Ok(_) => info!(?path, "Reloaded template"),
                    Err(err) => {
                        warn!(?err, "Failed to reload templates");
                        continue;
                    }
                }

                reload_sender.send(()).ok();
            } else if path.to_string_lossy().contains("content") {
                match compiler.try_reload_asset(&path).await {
                    Ok(true) => {
                        reload_sender.send(()).ok();
                        info!(?path, "Reloaded asset")
                    }
                    Ok(false) => (), // not referenced in any pages asset.
                    Err(err) => warn!(?err, "failed to reload asset"), // ehh, whatev
                }
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

fn is_djot(p: &Path) -> bool {
    let Some(ext) = p.extension() else {
        return false;
    };

    ext == "dj" || ext == "djot"
}

fn is_config(p: &Path) -> bool {
    let Some(ext) = p.extension() else {
        return false;
    };

    ext == "toml"
}

async fn serve(
    tx: broadcast::Sender<()>,
    build_path: &Path,
    static_assets_path: &Path,
    port: u16,
) -> eyre::Result<()> {
    info!(?build_path, ?static_assets_path);
    let static_assets_router: Router<()> = Router::new().fallback_service(
        ServeDir::new(build_path.join("static")).fallback(ServeDir::new(static_assets_path)),
    );

    let app = Router::new()
        // Hot reloading route. We use an SSE event with some additional javascript on the client to
        // achieve this. We also include the script needed to reload.
        .route("/__reload__", axum::routing::get(reload_sse))
        .route("/reload-script.js", axum::routing::get(reload_script))
        .nest_service("/static/", static_assets_router)
        .layer(CompressionLayer::new())
        .fallback_service(ServeDir::new(&build_path))
        .with_state(tx);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("dev server started on {}", listener.local_addr().unwrap());
    axum::serve(listener, app.layer(TraceLayer::new_for_http())).await?;
    Ok(())
}
