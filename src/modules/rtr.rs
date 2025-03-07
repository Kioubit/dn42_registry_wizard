use std::cell::Cell;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

use crate::modules::util::os_signals::{signal_listener, CustomSignal};
use crate::modules::util::BoxResult;
use rpki::resources::{
    addr::Prefix, Asn,
    MaxLenPrefix
};
use rpki::rtr::server::{PayloadDiff, PayloadSet};
use rpki::rtr::{payload::{Action, RouteOrigin}, server::{NotifySender, PayloadSource, Server}, state::State, PayloadRef, Timing};
use tokio::sync::broadcast::channel;
use tokio_stream::wrappers::TcpListenerStream;

#[derive(Clone)]
struct DataState {
    data: Arc<Mutex<Cell<Vec<RouteOrigin>>>>,
    state: Arc<Mutex<State>>,
}

impl DataState {
    fn new() -> Self {
        DataState{
            data: Arc::new(Mutex::new(Cell::new(Vec::new()))),
            state: Arc::new(Mutex::new(State::new())),
        }
    }
}

#[derive(Clone)]
struct VrpSource {
    data_state: Arc<DataState>,
}

impl VrpSource {
    fn new(data_state: Arc<DataState>) -> Self {
        VrpSource {
            data_state,
        }
    }
}

struct PayloadIterator {
    data: Arc<Mutex<Cell<Vec<RouteOrigin>>>>,
    position: usize
}

impl PayloadIterator {
    fn new(data: Arc<Mutex<Cell<Vec<RouteOrigin>>>>) -> Self {
        PayloadIterator { data, position: 0 }
    }
}

impl PayloadSet for PayloadIterator {
    fn next(&mut self) -> Option<PayloadRef<'_>> {
        let mut lock = self.data.lock().unwrap();
        self.position += 1;
        lock.get_mut().get(self.position).map(|d| PayloadRef::Origin(*d))
    }
}

impl PayloadDiff for PayloadIterator {
    fn next(&mut self) -> Option<(PayloadRef, Action)> {
        // Future work
        unimplemented!()
    }
}


impl PayloadSource for VrpSource {
    type Set = PayloadIterator;
    type Diff = PayloadIterator;

    fn ready(&self) -> bool {
        println!("READY CALLED");
        //self.ready
        true
    }

    fn notify(&self) -> State {
        *self.data_state.state.lock().unwrap()
    }

    fn full(&self) -> (State, Self::Set) {
        let state = self.data_state.state.lock().unwrap();
        let current_state = *state; // Copy the current state
        let iter = PayloadIterator::new(self.data_state.data.clone());
        println!("FULL CALLED");
        (current_state, iter)
    }

    fn diff(&self, _state: State) -> Option<(State, Self::Diff)> {
        println!("DIFF CALLED");
        None
    }

    fn timing(&self) -> Timing {
        Timing {
            refresh: 3600,
            retry: 600,
            expire: 7200,
        }
    }
}


pub fn start_rtr(registry_root: impl AsRef<Path>, port: u16) -> BoxResult<String>  {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let ds = Arc::new(DataState::new());
        match update_registry_data(registry_root.as_ref().to_path_buf()) {
            Err(err) => {
                eprintln!("Error updating registry data: {}", err);
            }
            Ok(result) => {
                ds.state.lock().unwrap().inc();
                ds.data.lock().unwrap().set(result);
            }
        }
        let notify = NotifySender::new();
        let vrp_source = VrpSource::new(ds.clone());
        let (sig_chan_tx, mut sig_chan_rx) = channel::<CustomSignal>(1);
        let signal_listener_handle = tokio::spawn(signal_listener(sig_chan_tx));

        let registry_root = registry_root.as_ref().to_path_buf();

        let mut notify2 = notify.clone();
        let registry_data_updater = tokio::spawn(async move {
            loop {
                match sig_chan_rx.recv().await.unwrap() {
                    CustomSignal::Shutdown => {
                        break;
                    }
                    CustomSignal::DataUpdate => {
                        eprintln!("Registry data update triggered");
                        match update_registry_data(registry_root.clone()) {
                            Err(err) => {
                                eprintln!("Error updating registry data: {}", err);
                            }
                            Ok(result) => {
                                ds.state.lock().unwrap().inc();
                                ds.data.lock().unwrap().set(result);
                                notify2.notify()
                            }
                        }
                        eprintln!("Registry data update completed")
                    }
                }
            }
            exit(0);
            Ok(())
        });

        let server = tokio::spawn(server(notify, vrp_source, port));
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

async fn server(notify: NotifySender, vrp_source: VrpSource, port: u16,) -> BoxResult<()> {
    let addr = SocketAddr::from((IpAddr::from(Ipv6Addr::UNSPECIFIED), port));
    let listener = TcpListener::bind(&addr).await?;
    println!("Listening on {}", addr);
    let listener_stream = TcpListenerStream::new(listener);
    let server = Server::new(listener_stream, notify, vrp_source);
    server.run().await?;
    Ok(())
}

fn update_registry_data(registry_root: PathBuf)  -> BoxResult<Vec<RouteOrigin>> {
    let roa4 = roa_wizard_lib::get_roa_objects(false, registry_root.clone()).map_err(|x| {
        format!("Error generating roa4: {:?}", x)
    }).map(|x| x.0)?;
    let roa6 = roa_wizard_lib::get_roa_objects(true, registry_root.clone()).map_err(|x| {
        format!("Error generating roa6: {:?}", x)
    }).map(|x| x.0)?;

    let mut result = Vec::new();
    for item in roa4.iter().chain(roa6.iter()) {
        let ip_addr = item.prefix.first_address();
        let prefix_length = item.prefix.network_length();
        let prefix = Prefix::new(ip_addr,prefix_length)?;
        let max_len_prefix = MaxLenPrefix::new(prefix, Some(item.max_length.get().unwrap() as u8))?;
        for origin in &item.origins {
            let asn = Asn::from_u32(origin.parse()?);
            result.push(RouteOrigin { prefix: max_len_prefix, asn });
        }
    }
    Ok(result)
}