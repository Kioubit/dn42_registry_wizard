use std::collections::VecDeque;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

use crate::modules::util::os_signals::{signal_listener, CustomSignal};
use crate::modules::util::BoxResult;
use rpki::resources::{addr::Prefix, Asn, MaxLenPrefix};
use rpki::rtr::server::{PayloadDiff, PayloadSet};
use rpki::rtr::{
    payload::{Action, RouteOrigin},
    server::{NotifySender, PayloadSource, Server},
    state::State,
    PayloadRef, Timing,
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::TcpListenerStream;

struct RecentItems<T> {
    buffer: VecDeque<T>,
    capacity: usize,
}

impl<T> RecentItems<T> {
    fn new(capacity: usize) -> Self {
        RecentItems {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn add(&mut self, item: T) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    fn latest(&self) -> Option<&T> {
        self.buffer.back()
    }

    fn iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.buffer.iter()
    }
}

type StateWithData = RecentItems<Arc<(State, Vec<RouteOrigin>)>>;
#[derive(Clone)]
struct DataState {
    data: Arc<Mutex<StateWithData>>,
}

impl DataState {
    fn new() -> Self {
        DataState {
            data: Arc::new(Mutex::new(RecentItems::new(4))),
        }
    }

    fn add_data(&self, new: Vec<RouteOrigin>) {
        let mut data = self.data.lock().unwrap();
        let new_state = if let Some(mut latest_state) = data.latest().map(|x| x.clone().0) {
            latest_state.inc();
            latest_state
        } else {
            State::new()
        };
        data.add(Arc::new((new_state, new)))
    }
}

#[derive(Clone)]
struct VrpSource {
    data_state: Arc<DataState>,
    timings: Timing,
}

impl VrpSource {
    fn new(data_state: Arc<DataState>, timings: Timing) -> Self {
        VrpSource {
            data_state,
            timings,
        }
    }
}

struct PayloadIterator {
    latest_data: Arc<(State, Vec<RouteOrigin>)>,
    old_data: Option<Arc<(State, Vec<RouteOrigin>)>>,
    position: usize,
}

impl PayloadIterator {
    fn new(
        latest_data: Arc<(State, Vec<RouteOrigin>)>,
        old_data: Option<Arc<(State, Vec<RouteOrigin>)>>,
    ) -> Self {
        PayloadIterator {
            latest_data,
            old_data,
            position: 0,
        }
    }
}

impl PayloadSet for PayloadIterator {
    fn next(&mut self) -> Option<PayloadRef<'_>> {
        self.position += 1;
        self.latest_data
            .1
            .get(self.position)
            .map(|d| PayloadRef::Origin(*d))
    }
}

impl PayloadDiff for PayloadIterator {
    fn next(&'_ mut self) -> Option<(PayloadRef<'_>, Action)> {
        // Get references to the data vectors
        let latest_origins = &self.latest_data.1;

        while self.position < latest_origins.len() {
            // Process the current item from latest_data
            let current = &latest_origins[self.position];
            self.position += 1;

            // Determine if this is an announcement or withdrawal
            if let Some(old_data) = &self.old_data {
                let old_origins = &old_data.1;

                // Check if this route origin existed in the old data
                let existed = old_origins.iter().any(|ro| ro == current);
                if !existed {
                    return Some((PayloadRef::from(current), Action::Announce));
                }
            } else {
                // If no old data, everything is an announcement
                return Some((PayloadRef::from(current), Action::Announce));
            };
        }

        // If we've gone through all latest entries
        // If old_data exists, we need to check for withdrawals
        if let Some(old_data) = &self.old_data {
            // Find entries in old_data that aren't in latest_data
            // We start from self.position - latest_origins.len() to account for already processed items
            let old_origins = &old_data.1;
            loop {
                let old_index = self.position - latest_origins.len();
                self.position += 1;

                // If we've gone through all old entries too, we're done
                if old_index >= old_origins.len() {
                    return None;
                }

                let old_origin = &old_origins[old_index];

                // Check if this old entry exists in latest_data
                let exists_in_latest = latest_origins.iter().any(|ro| ro == old_origin);

                // If it doesn't exist in latest, it's a withdrawal
                if !exists_in_latest {
                    return Some((PayloadRef::from(old_origin), Action::Withdraw));
                }
            }
        }

        None
    }
}

impl PayloadSource for VrpSource {
    type Set = PayloadIterator;
    type Diff = PayloadIterator;

    fn ready(&self) -> bool {
        self.data_state.data.lock().unwrap().latest().is_some()
    }

    fn notify(&self) -> State {
        let d = self.data_state.data.lock().unwrap();
        d.latest().unwrap().0
    }

    fn full(&self) -> (State, Self::Set) {
        println!("Received full VRP set request");
        let d = self.data_state.data.lock().unwrap();
        let latest = d.latest().unwrap().clone();
        drop(d);
        let current_state = latest.0;
        let iter = PayloadIterator::new(latest, None);
        (current_state, iter)
    }

    fn diff(&self, state: State) -> Option<(State, Self::Diff)> {
        println!("Received differential VRP set request");
        let d = self.data_state.data.lock().unwrap();
        let latest = d.latest().unwrap().clone();
        let current_state = latest.0;

        let mut old_data = None;
        for s in d.iter() {
            let test_state = s.0;
            if test_state.serial() == state.serial() {
                old_data = Some(s.clone());
                break;
            }
        }
        drop(d);
        if old_data.is_some() {
            Some((current_state, PayloadIterator::new(latest, old_data)))
        } else {
            None
        }
    }

    fn timing(&self) -> Timing {
        self.timings
    }
}

pub fn start_rtr(
    registry_root: impl AsRef<Path>,
    port: u16,
    refresh: u32,
    retry: u32,
    expire: u32,
) -> BoxResult<String> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let ds = Arc::new(DataState::new());
        match update_registry_data(registry_root.as_ref().to_path_buf()) {
            Err(err) => {
                eprintln!("Error updating registry data: {}", err);
            }
            Ok(result) => ds.add_data(result),
        }
        let notify = NotifySender::new();
        let vrp_source = VrpSource::new(
            ds.clone(),
            Timing {
                refresh,
                retry,
                expire,
            },
        );
        let (sig_chan_tx, mut sig_chan_rx) = broadcast::channel::<CustomSignal>(1);
        let signal_listener_handle = tokio::spawn(signal_listener(sig_chan_tx.clone()));

        let registry_root = registry_root.as_ref().to_path_buf();

        let mut notify_clone = notify.clone();
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
                                ds.add_data(result);
                                notify_clone.notify()
                            }
                        }
                        eprintln!("Registry data update completed")
                    }
                }
            }
            Ok(())
        });

        let server = tokio::spawn(server(notify, vrp_source, port, sig_chan_tx.subscribe()));
        let result = tokio::try_join!(
            async { registry_data_updater.await? },
            async { server.await? },
            async { signal_listener_handle.await? }
        );
        if let Err(e) = result {
            return Err(format!("Error: {}", e));
        }
        Ok(())
    })?;
    Ok("".into())
}

async fn server(
    notify: NotifySender,
    vrp_source: VrpSource,
    port: u16,
    mut signal_rx: broadcast::Receiver<CustomSignal>,
) -> BoxResult<()> {
    let addr = SocketAddr::from((IpAddr::from(Ipv6Addr::UNSPECIFIED), port));
    let listener = TcpListener::bind(&addr).await?;
    println!(
        "Listening on {}. Send the POSIX 'SIGUSR1' signal to this process to trigger data update",
        addr
    );
    let listener_stream = TcpListenerStream::new(listener);
    let server = Server::new(listener_stream, notify, vrp_source);
    let result = tokio::select! {
        res = server.run() => {
            if let Err(e) = res {
                Err(format!("Server error: {}", e).into())
            } else {
                Ok(())
            }
        }
        _ = async {
            loop {
                if let Ok(CustomSignal::Shutdown) = signal_rx.recv().await {
                    break;
                }
            }
        } => {
            Ok(())
        }
    };
    result
}

fn update_registry_data(registry_root: PathBuf) -> BoxResult<Vec<RouteOrigin>> {
    let roa4 = roa_wizard_lib::get_roa_objects(false, registry_root.clone())
        .map_err(|x| format!("Error generating roa4: {:?}", x))
        .map(|x| x.0)?;
    let roa6 = roa_wizard_lib::get_roa_objects(true, registry_root.clone())
        .map_err(|x| format!("Error generating roa6: {:?}", x))
        .map(|x| x.0)?;

    let mut result = Vec::new();
    for item in roa4.iter().chain(roa6.iter()) {
        let ip_addr = item.prefix.first_address();
        let prefix_length = item.prefix.network_length();
        let prefix = Prefix::new(ip_addr, prefix_length)?;
        let max_len_prefix = MaxLenPrefix::new(prefix, Some(item.max_length.get().unwrap() as u8))?;
        for origin in &item.origins {
            let asn = Asn::from_u32(origin.parse()?);
            result.push(RouteOrigin {
                prefix: max_len_prefix,
                asn,
            });
        }
    }
    Ok(result)
}
