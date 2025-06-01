use tokio::select;
use tokio::sync::broadcast;
use crate::modules::util::BoxResult;


#[derive(Debug, Clone)]
pub enum CustomSignal {
    Shutdown,
    DataUpdate,
}

#[cfg(unix)]
pub(in crate::modules) async fn signal_listener(sig_chan_tx: broadcast::Sender<CustomSignal>) -> BoxResult<()> {
    use tokio::signal::unix::{signal, SignalKind};
    if let Ok(mut user1_signal) = signal(SignalKind::user_defined1()) {
        loop {
            select! {
                _ = user1_signal.recv() => {
                    sig_chan_tx.send(CustomSignal::DataUpdate).unwrap();
                }
                _ = terminate_signal() => {
                    sig_chan_tx.send(CustomSignal::Shutdown).unwrap();
                    break;
                }
            }
        }
    } else {
        eprintln!("Error registering user_defined1 signal");
        select! {
            _ = terminate_signal() => {
                sig_chan_tx.send(CustomSignal::Shutdown).unwrap();
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
pub(in crate::modules) async fn signal_listener(sig_chan_tx: broadcast::Sender<CustomSignal>) -> BoxResult<()>{
    select! {
            _ = terminate_signal() => {
                sig_chan_tx.send(CustomSignal::Shutdown).unwrap();
            }
    }
    Ok(())
}

#[cfg(unix)]
pub(in crate::modules) async fn terminate_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    select! {
        _ = sigterm.recv() => (),
        _ = sigint.recv() => ()
    }
}

#[cfg(windows)]
pub(in crate::modules) async fn terminate_signal() {
    use tokio::signal::windows::ctrl_c;
    let mut ctrl_c = ctrl_c().unwrap();
    let _ = ctrl_c.recv().await;
}