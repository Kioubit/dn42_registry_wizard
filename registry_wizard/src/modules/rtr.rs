use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;

use crate::modules::util::BoxResult;
use crate::modules::util::os_signals::{CustomSignal, signal_listener};
use rpki::resources::{Asn, MaxLenPrefix, addr::Prefix};
use rpki::rtr::server::{PayloadDiff, PayloadSet};
use rpki::rtr::{
    PayloadRef, Timing,
    payload::{Action, RouteOrigin},
    server::{NotifySender, PayloadSource, Server},
    state::State,
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::TcpListenerStream;


// A single generation of VRP data
struct Generation {
    state: State,
    data: Vec<RouteOrigin>,
}

struct RecentGenerations {
    buffer: VecDeque<Arc<Generation>>,
    capacity: usize,
}

impl RecentGenerations {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        RecentGenerations {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn add(&mut self, item: Arc<Generation>) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(item);
    }

    fn latest(&self) -> Option<&Arc<Generation>> {
        self.buffer.back()
    }

    fn find_serial(&self, serial: rpki::rtr::state::Serial) -> Option<&Arc<Generation>> {
        self.buffer.iter().find(|g| g.as_ref().state.serial() == serial)
    }
}

type GenerationDiff = (RouteOrigin, Action);

struct Inner {
    history: RecentGenerations,
    /// Memoized diffs: old_serial -> diff against the current latest generation
    /// Cleared whenever a new generation is added
    diff_cache: HashMap<rpki::rtr::state::Serial, Arc<Vec<GenerationDiff>>>,
}

#[derive(Clone)]
struct DataState {
    inner: Arc<RwLock<Inner>>,
}

impl DataState {
    fn new(history_size: usize) -> Self {
        DataState {
            inner: Arc::new(RwLock::new(Inner {
                history: RecentGenerations::new(history_size),
                diff_cache: HashMap::new(),
            })),
        }
    }

    /// Adds a new generation if it differs from the current latest
    /// Returns true if a new generation was actually added
    fn add_data(&self, new: Vec<RouteOrigin>) -> bool {
        let mut inner = self.inner.write().unwrap();

        if let Some(latest) = inner.history.latest() {
            let old_set: HashSet<_> = latest.data.iter().copied().collect();
            let new_set: HashSet<_> = new.iter().copied().collect();
            if old_set == new_set {
                return false;
            }
        }

        let new_state = if let Some(latest) = inner.history.latest() {
            let mut s = latest.state;
            s.inc();
            s
        } else {
            State::new()
        };

        inner.history.add(Arc::new(Generation { state: new_state, data: new }));
        // Any cached diff was computed relative to the old latest and is now stale
        inner.diff_cache.clear();
        true
    }
    fn latest(&self) -> Option<Arc<Generation>> {
        self.inner.read().unwrap().history.latest().cloned()
    }

    /// Returns (current_state, diff) if `old_state` is still in the history
    /// Computes and caches the diff on first request for re-use
    fn diff_from(&self, old_state: State) -> Option<(State, Arc<Vec<GenerationDiff>>)> {
        // Cached
        {
            let inner = self.inner.read().unwrap();
            let latest = inner.history.latest()?;
            if latest.state.serial() == old_state.serial() {
                return Some((latest.state, Arc::new(Vec::new())));
            }
            if let Some(diff) = inner.diff_cache.get(&old_state.serial()) {
                return Some((latest.state, diff.clone()));
            }
        }

        // Not cached: Compute and store
        let mut inner = self.inner.write().unwrap();
        let latest = inner.history.latest()?.clone();

        // Someone else might have computed it while waiting for the write lock
        if let Some(diff) = inner.diff_cache.get(&old_state.serial()) {
            return Some((latest.state, diff.clone()));
        }

        let old = inner.history.find_serial(old_state.serial())?.clone();
        let diff = Arc::new(compute_diff(&old.data, &latest.data));
        inner.diff_cache.insert(old_state.serial(), diff.clone());
        Some((latest.state, diff))
    }
}

fn compute_diff(old: &[RouteOrigin], new: &[RouteOrigin]) -> Vec<GenerationDiff> {
    let old_set: HashSet<_> = old.iter().copied().collect();
    let new_set: HashSet<_> = new.iter().copied().collect();
    let mut result = Vec::new();
    result.extend(new.iter().filter(|i| !old_set.contains(i)).map(|i| (*i, Action::Announce)));
    result.extend(old.iter().filter(|i| !new_set.contains(i)).map(|i| (*i, Action::Withdraw)));
    result
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

struct FullIterator {
    data: Arc<Generation>,
    position: usize,
}

impl PayloadSet for FullIterator {
    fn next(&mut self) -> Option<PayloadRef<'_>> {
        let item = self.data.data.get(self.position)?;
        self.position += 1;
        Some(PayloadRef::Origin(*item))
    }
}

struct DiffIterator {
    diff: Arc<Vec<GenerationDiff>>,
    position: usize,
}

impl PayloadDiff for DiffIterator {
    fn next(&mut self) -> Option<(PayloadRef<'_>, Action)> {
        let (origin, action) = self.diff.get(self.position)?;
        self.position += 1;
        Some((PayloadRef::from(origin), *action))
    }
}

impl PayloadSource for VrpSource {
    type Set = FullIterator;
    type Diff = DiffIterator;

    fn ready(&self) -> bool {
        self.data_state.latest().is_some()
    }

    fn notify(&self) -> State {
        self.data_state.latest().unwrap().state
    }

    fn full(&self) -> (State, Self::Set) {
        println!("Received full VRP set request");
        let latest = self.data_state.latest().unwrap();
        let state = latest.state;
        (state, FullIterator { data: latest, position: 0 })
    }

    fn diff(&self, state: State) -> Option<(State, Self::Diff)> {
        println!("Received differential VRP set request");
        let (current, diff) = self.data_state.diff_from(state)?;
        Some((current, DiffIterator { diff, position: 0 }))
    }

    fn timing(&self) -> Timing {
        self.timings
    }
}

pub fn start_rtr(
    registry_root: impl AsRef<Path>,
    port: u16,
    bind_ip: Option<String>,
    refresh: u32,
    retry: u32,
    expire: u32,
    history_size: usize,
) -> BoxResult<String> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let ds = Arc::new(DataState::new(history_size));
        match update_registry_data(registry_root.as_ref()) {
            Err(err) => {
                eprintln!("Error updating registry data: {}", err);
            }
            Ok(result) => {
                ds.add_data(result);
            },
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

        let registry_root = registry_root.as_ref().to_owned();

        let mut notify_clone = notify.clone();
        let registry_data_updater = tokio::spawn(async move {
            loop {
                match sig_chan_rx.recv().await {
                    Ok(CustomSignal::Shutdown) => {
                        break;
                    }
                    Ok(CustomSignal::DataUpdate) => {
                        eprintln!("Registry data update triggered");
                        match update_registry_data(registry_root.clone().as_ref()) {
                            Err(err) => {
                                eprintln!("Error updating registry data: {}", err);
                            }
                            Ok(result) => {
                                if ds.add_data(result) {
                                    notify_clone.notify();
                                } else {
                                    eprintln!("Registry data unchanged, skipping notification");
                                }
                            }
                        }
                        eprintln!("Registry data update completed")
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("Signal channel lagged, missed {n} messages, continuing");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            Ok(())
        });

        let server = tokio::spawn(server(notify, vrp_source, port, bind_ip, sig_chan_tx.subscribe()));
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
    bind_ip: Option<String>,
    mut signal_rx: broadcast::Receiver<CustomSignal>,
) -> BoxResult<()> {
    let bind_ip = if let Some(bind_ip) = bind_ip {
        IpAddr::from_str(bind_ip.as_str())?
    } else {
        IpAddr::from(Ipv6Addr::UNSPECIFIED)
    };

    let addr = SocketAddr::from((bind_ip, port));
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
                match signal_rx.recv().await {
                    Ok(CustomSignal::Shutdown) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        } => {
            Ok(())
        }
    };
    result
}

fn update_registry_data(registry_root: &Path) -> BoxResult<Vec<RouteOrigin>> {
    let roa = roa_wizard::get_roa_data_combined(registry_root, |warn|{
        eprintln!("Warning during ROA data generation: {}", warn);
        roa_wizard::WarningAction::ActionContinue
    }).map_err(|x| format!("Error generating roa: {}", x))?;

    let mut result = Vec::new();
    for item in roa.object_list() {
        let prefix = Prefix::new(item.prefix.first_address(), item.prefix.network_length())?;
        let max_len = item.max_length.unwrap_or(item.prefix.network_length());

        let m_prefix = MaxLenPrefix::new(prefix, Some(max_len))?;

        for origin in &item.origins {
            let asn = Asn::from_u32(origin.parse()?);
            result.push(RouteOrigin {
                prefix: m_prefix,
                asn,
            });
        }
    }
    Ok(result)
}
