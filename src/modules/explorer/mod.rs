use crate::modules::util::BoxResult;
use axum::routing::get;
use axum::Router;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tokio::sync::broadcast::channel;
use crate::modules::explorer::os_signals::signal_listener;
use crate::modules::explorer::state::AppState;

mod static_files;
mod handlers;
mod state;
mod os_signals;

#[derive(Debug, Clone)]
enum CustomSignal {
    Shutdown,
    DataUpdate,
}

pub fn start_explorer(registry_root: impl AsRef<Path>, port: u16, with_roa: bool) -> BoxResult<String> {
    let registry_root: PathBuf = registry_root.as_ref().to_owned();
    let app_state = Arc::new(RwLock::new(Default::default()));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        if let Err(err) = state::update_registry_data(registry_root.clone(), app_state.clone(), with_roa).await {
            return Err(format!("Error reading registry data: {}", err));
        }
        if !with_roa {
            app_state.write().unwrap().roa_disabled  = true;
            const MSG_ROA_DISABLED: &str = "ROA data generation disabled";
            app_state.write().unwrap().roa4 = Some(String::from(MSG_ROA_DISABLED));
            app_state.write().unwrap().roa6 = Some(String::from(MSG_ROA_DISABLED));
            app_state.write().unwrap().roa_json = Some(String::from(MSG_ROA_DISABLED));
        }

        let (sig_chan_tx, mut sig_chan_rx) = channel::<CustomSignal>(1);
        let signal_listener_handle = tokio::spawn(signal_listener(sig_chan_tx));


        let sig_chan_rx_2 = sig_chan_rx.resubscribe();
        let app_state_clone = app_state.clone();
        let registry_data_updater = tokio::spawn(async move {
            loop {
                match sig_chan_rx.recv().await.unwrap() {
                    CustomSignal::Shutdown => {
                        break;
                    }
                    CustomSignal::DataUpdate => {
                        eprintln!("Registry data update triggered");
                        if let Err(err) = state::update_registry_data(registry_root.clone(), app_state.clone(), with_roa).await {
                            eprintln!("Error updating registry data: {}", err);
                        }
                        eprintln!("Registry data update completed")
                    }
                }
            }
            Ok(())
        });

        let server = tokio::spawn(start_server(app_state_clone, port, sig_chan_rx_2));
        let result = tokio::try_join!(
            async {registry_data_updater.await?},
            async {server.await?},
            async {signal_listener_handle.await?}
        );
        if let Err(e) = result {
            return Err(format!("Error: {}", e));
        }
        Ok(())
    })?;

    Ok("".into())
}


async fn start_server(app_state: Arc<RwLock<AppState>>, port: u16, mut sig_chan_rx: broadcast::Receiver<CustomSignal>) -> BoxResult<()> {
    let addr = SocketAddr::from((IpAddr::from(Ipv6Addr::UNSPECIFIED), port));
    let listener = tokio::net::TcpListener::bind(&addr).await
        .map_err(|x| format!("Error listening on TCP: {}", x))?;
    let app = Router::new()
        .route("/", get(handlers::root_handler))
        .route("/*path", get(handlers::root_handler))
        .route("/api/index/", get(handlers::index_handler))
        .route("/api/object/", get(handlers::get_object))
        .route("/api/roa/v4/", get(handlers::roa_handler_v4))
        .route("/api/roa/v6/", get(handlers::roa_handler_v6))
        .route("/api/roa/json/", get(handlers::roa_handler_json))
        .with_state(app_state);

    eprintln!("Starting server on port {}. Send the POSIX 'SIGUSR1' signal to this process to trigger data update", port);
    eprintln!("ROA data endpoints: ['/api/roa/v4/', '/api/roa/v6/', '/api/roa/json/']");
    axum::serve(listener, app).with_graceful_shutdown(async move {
        loop {
            match sig_chan_rx.recv().await.unwrap() {
                CustomSignal::Shutdown => { break }
                CustomSignal::DataUpdate => {}
            }
        }
    }).await
        .map_err(|e| format!("Error starting server: {}", e))?;
    Ok(())
}
