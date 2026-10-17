use crate::{
    model::*,
    pdf_worker::{read_pixels, write_json},
};
use std::{
    collections::HashSet,
    io::{BufReader, BufWriter},
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub session: u64,
    pub revision: u64,
    pub path: String,
    pub jobs: Vec<Key>,
}
struct Active {
    session: u64,
    key: Key,
    started: Instant,
    cancelled: bool,
    timed_out: bool,
}
#[derive(Default)]
struct Shared {
    plan: Option<Plan>,
    active: Option<Active>,
    stop: bool,
}
#[derive(Clone, Default, serde::Serialize)]
pub struct Metrics {
    pub rendered: u64,
    pub cache_hits: u64,
    pub cancellations: u64,
    pub cancel_kill_ms: Vec<f64>,
    pub restarts: u64,
    pub stale_dropped: u64,
    pub cpu_cache_peak: usize,
    pub worker_errors: u64,
}
pub struct Reply {
    pub session: u64,
    pub pixels: Arc<Pixels>,
}
pub enum Event {
    Ready(Reply),
    Error { session: u64, message: String },
}
pub struct Controller {
    shared: Arc<Mutex<Shared>>,
    pub rx: mpsc::Receiver<Event>,
    metrics: Arc<Mutex<Metrics>>,
    threads: Vec<thread::JoinHandle<()>>,
}
impl Controller {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let metrics = Arc::new(Mutex::new(Metrics::default()));
        let child = Arc::new(Mutex::new(None::<Child>));
        let (tx, rx) = mpsc::sync_channel(2);
        let wake = Arc::new(wake);
        let monitor = {
            let state = shared.clone();
            let child = child.clone();
            let metrics = metrics.clone();
            thread::spawn(move || loop {
                // A separate monitor can kill a native call while the supervisor
                // blocks on its reply. No PDFium handles exist in this process.
                let kill = {
                    let mut s = state.lock().unwrap();
                    if s.stop {
                        if let Some(c) = child.lock().unwrap().as_mut() {
                            let _ = c.kill();
                        }
                        break;
                    }
                    let immediate = s
                        .active
                        .as_ref()
                        .is_some_and(|a| s.plan.as_ref().is_some_and(|p| p.session != a.session));
                    let unwanted = s.active.as_ref().is_some_and(|a| {
                        s.plan
                            .as_ref()
                            .is_none_or(|p| p.session != a.session || !p.jobs.contains(&a.key))
                    });
                    let timed_out = s
                        .active
                        .as_ref()
                        .is_some_and(|a| a.started.elapsed() > Duration::from_secs(15));
                    if unwanted || timed_out {
                        let a = s.active.as_mut().unwrap();
                        if !a.cancelled
                            && (immediate
                                || timed_out
                                || a.started.elapsed() > Duration::from_millis(80))
                        {
                            a.cancelled = true;
                            a.timed_out = timed_out;
                            Some(a.started)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some(started) = kill {
                    // Recheck under the state lock so an old cancellation can
                    // never kill a replacement process after the reply races it.
                    let s = state.lock().unwrap();
                    if !s
                        .active
                        .as_ref()
                        .is_some_and(|a| a.started == started && a.cancelled)
                    {
                        continue;
                    }
                    let t = Instant::now();
                    if let Some(c) = child.lock().unwrap().as_mut() {
                        let _ = c.kill();
                    }
                    let mut m = metrics.lock().unwrap();
                    m.cancellations += 1;
                    m.cancel_kill_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                thread::sleep(Duration::from_millis(5));
            })
        };
        let supervisor = {
            let state = shared.clone();
            let metrics = metrics.clone();
            thread::spawn(move || {
                let mut cache: Cache<Key, Arc<Pixels>> = Cache::new(CPU_BUDGET);
                let mut process = None;
                let mut revision = (0, 0);
                let mut done = HashSet::new();
                let mut seq = 0;
                loop {
                    let plan = {
                        let s = state.lock().unwrap();
                        if s.stop {
                            break;
                        }
                        s.plan.clone()
                    };
                    let Some(plan) = plan else {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    if revision != (plan.session, plan.revision) {
                        if revision.0 != plan.session {
                            done.clear();
                        } else {
                            done.retain(|key| plan.jobs.contains(key));
                        }
                        revision = (plan.session, plan.revision);
                    }
                    let Some(key) = plan.jobs.iter().find(|k| !done.contains(*k)).cloned() else {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    done.insert(key.clone());
                    let cached = cache.get(&key).cloned();
                    let mut interrupted = false;
                    let result = if let Some(p) = cached {
                        metrics.lock().unwrap().cache_hits += 1;
                        Ok(p)
                    } else {
                        state.lock().unwrap().active = Some(Active {
                            session: plan.session,
                            key: key.clone(),
                            started: Instant::now(),
                            cancelled: false,
                            timed_out: false,
                        });
                        seq += 1;
                        let mut result = (|| -> Result<Arc<Pixels>, String> {
                            if process
                                .as_ref()
                                .is_none_or(|p: &Process| p.session != plan.session)
                            {
                                stop_child(&child);
                                process = None;
                                let mut c = Command::new(
                                    std::env::current_exe().map_err(|e| e.to_string())?,
                                )
                                .arg("--pdf-worker")
                                .arg(&plan.path)
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::inherit())
                                .spawn()
                                .map_err(|e| e.to_string())?;
                                let input = BufWriter::new(c.stdin.take().unwrap());
                                let output = BufReader::new(c.stdout.take().unwrap());
                                *child.lock().unwrap() = Some(c);
                                process = Some(Process {
                                    session: plan.session,
                                    input,
                                    output,
                                });
                                metrics.lock().unwrap().restarts += 1;
                            }
                            let process = process.as_mut().unwrap();
                            let req = Request {
                                seq,
                                key: key.clone(),
                            };
                            write_json(&req, &mut process.input)?;
                            let pixels = read_pixels(&mut process.output, &req)?;
                            if let Some(e) = &pixels.header.error {
                                return Err(e.clone());
                            }
                            Ok(Arc::new(pixels))
                        })();
                        let active = state.lock().unwrap().active.take();
                        let cancelled = active.as_ref().is_some_and(|a| a.cancelled);
                        let timed_out = active.as_ref().is_some_and(|a| a.timed_out);
                        interrupted = cancelled && !timed_out;
                        if timed_out {
                            result =
                                Err("PDF worker exceeded its 15-second request deadline".into());
                        }
                        if result.is_err() || cancelled {
                            stop_child(&child);
                            process = None;
                        }
                        result
                    };
                    if interrupted {
                        metrics.lock().unwrap().stale_dropped += 1;
                        done.remove(&key);
                        continue;
                    }
                    if !wanted(&state, &plan, &key) {
                        metrics.lock().unwrap().stale_dropped += 1;
                        continue;
                    }
                    let event = match result {
                        Ok(pixels) => {
                            if (key.level >= -1 || key.level == -3) && !cache.contains(&key) {
                                cache.insert(key.clone(), pixels.clone(), pixels.rgba.len());
                                let mut m = metrics.lock().unwrap();
                                m.rendered += 1;
                                m.cpu_cache_peak = cache.peak;
                            }
                            Event::Ready(Reply {
                                session: plan.session,
                                pixels,
                            })
                        }
                        Err(message) => {
                            metrics.lock().unwrap().worker_errors += 1;
                            Event::Error {
                                session: plan.session,
                                message,
                            }
                        }
                    };
                    let mut pending = event;
                    loop {
                        if !wanted(&state, &plan, &key) {
                            break;
                        }
                        match tx.try_send(pending) {
                            Ok(()) => {
                                wake();
                                break;
                            }
                            Err(mpsc::TrySendError::Full(e)) => {
                                pending = e;
                                thread::sleep(Duration::from_millis(5));
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => {
                                stop_child(&child);
                                return;
                            }
                        }
                    }
                }
                stop_child(&child);
            })
        };
        Self {
            shared,
            rx,
            metrics,
            threads: vec![monitor, supervisor],
        }
    }
    pub fn submit(&self, plan: Plan) {
        assert!(plan.jobs.len() <= MAX_JOBS);
        self.shared.lock().unwrap().plan = Some(plan);
    }
    pub fn metrics(&self) -> Metrics {
        self.metrics.lock().unwrap().clone()
    }
    pub fn is_busy(&self) -> bool {
        self.shared.lock().unwrap().active.is_some()
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.shared.lock().unwrap().stop = true;
        // Application shutdown waits for kill/reap so no PDF worker is orphaned.
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}
struct Process {
    session: u64,
    input: BufWriter<std::process::ChildStdin>,
    output: BufReader<std::process::ChildStdout>,
}
fn stop_child(child: &Arc<Mutex<Option<Child>>>) {
    if let Some(mut c) = child.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}
fn wanted(state: &Mutex<Shared>, plan: &Plan, key: &Key) -> bool {
    let s = state.lock().unwrap();
    !s.stop
        && s.plan
            .as_ref()
            .is_some_and(|p| p.session == plan.session && p.jobs.contains(key))
}
