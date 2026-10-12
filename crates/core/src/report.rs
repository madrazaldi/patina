//! An ordered worker for everything the player tells Plex.
//!
//! Timeline reports, scrobbles, stream choices and play-queue edits used to be
//! awaited while handling mpv's events, so one slow or failing request (the
//! shared HTTP timeout is 60 seconds) delayed queue edits and playback
//! updates. The player now only enqueues an [`Op`]; one task sends them in
//! order and keeps the backlog small while Plex is slow:
//!
//! * progress checkpoints for the same item collapse into the newest one;
//! * the final `stopped` report replaces the pending progress it supersedes and
//!   is never dropped (it, scrobbles and stream choices are retried);
//! * a progress report is never sent after a newer report of the same item was
//!   queued, and requests that hang are cut off after a timeout.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use patina_plex::{PlexServer, TimelineState, TimelineUpdate};
use tokio::sync::{mpsc, oneshot};

/// One thing to tell the server.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Timeline(TimelineUpdate),
    Scrobble { rating_key: String },
    Streams { part_id: i64, audio: Option<i64>, subtitle: Option<i64> },
    PlayQueue(QueueOp),
    PingTranscode(String),
    StopTranscode(String),
}

/// Edits mirrored to the server's play queue.
#[derive(Clone, Debug, PartialEq)]
pub enum QueueOp {
    Remove { queue: i64, item: i64 },
    Move { queue: i64, item: i64, after: Option<i64> },
    Shuffle { queue: i64 },
    Unshuffle { queue: i64 },
}

impl Op {
    /// The item a timeline report or scrobble is about.
    fn subject(&self) -> Option<&str> {
        match self {
            Op::Timeline(u) => Some(&u.rating_key),
            Op::Scrobble { rating_key } => Some(rating_key),
            _ => None,
        }
    }

    fn is_progress(&self) -> bool {
        matches!(self, Op::Timeline(u) if u.state != TimelineState::Stopped)
    }

    /// Worth retrying: losing it would leave the server with a wrong resume point or watch state.
    fn is_important(&self) -> bool {
        match self {
            Op::Timeline(u) => u.state == TimelineState::Stopped,
            Op::Scrobble { .. } | Op::Streams { .. } | Op::PlayQueue(_) | Op::StopTranscode(_) => true,
            Op::PingTranscode(_) => false,
        }
    }
}

/// An [`Op`] for one server (`T`; a [`PlexServer`] in the app).
#[derive(Clone, Debug)]
pub struct Job<T> {
    pub target: T,
    pub op: Op,
    attempts: u32,
    not_before: Option<Instant>,
}

/// The backlog, with coalescing applied as jobs are added.
pub struct Pending<T> {
    jobs: VecDeque<Job<T>>,
}

impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self { jobs: VecDeque::new() }
    }
}

impl<T: PartialEq> Pending<T> {
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn ops(&self) -> impl Iterator<Item = &Op> {
        self.jobs.iter().map(|j| &j.op)
    }

    pub fn push(&mut self, target: T, op: Op) {
        let job = Job { target, op, attempts: 0, not_before: None };
        match &job.op {
            Op::Timeline(_) => {
                // Fold into the last pending job about this item when that is
                // progress: only the newest checkpoint (or the final stop) matters.
                let last = self.jobs.iter().rposition(|j| j.target == job.target && j.op.subject() == job.op.subject());
                match last {
                    Some(i) if self.jobs[i].op.is_progress() => self.jobs[i] = Job { not_before: self.jobs[i].not_before, ..job },
                    _ => self.jobs.push_back(job),
                }
            }
            Op::Streams { part_id, audio, subtitle } => {
                let same = self.jobs.iter().position(|j| {
                    j.target == job.target && matches!(&j.op, Op::Streams { part_id: p, .. } if p == part_id)
                });
                match same {
                    Some(i) => {
                        if let Op::Streams { audio: a, subtitle: s, .. } = &mut self.jobs[i].op {
                            *a = audio.or(*a);
                            *s = subtitle.or(*s);
                        }
                    }
                    None => self.jobs.push_back(job),
                }
            }
            Op::PingTranscode(session) => {
                let queued = self.jobs.iter().any(|j| {
                    j.target == job.target && matches!(&j.op, Op::PingTranscode(s) | Op::StopTranscode(s) if s == session)
                });
                if !queued {
                    self.jobs.push_back(job);
                }
            }
            Op::StopTranscode(session) => {
                self.jobs.retain(|j| !(j.target == job.target && matches!(&j.op, Op::PingTranscode(s) if s == session)));
                self.jobs.push_back(job);
            }
            Op::Scrobble { .. } | Op::PlayQueue(_) => self.jobs.push_back(job),
        }
    }

    fn pop(&mut self) -> Option<Job<T>> {
        self.jobs.pop_front()
    }

    /// Requeue a failed job at the front, to keep the order it was sent in.
    fn retry(&mut self, mut job: Job<T>, delay: Duration) {
        job.attempts += 1;
        job.not_before = Some(Instant::now() + delay);
        self.jobs.push_front(job);
    }

    fn next_ready(&self) -> Option<Instant> {
        self.jobs.front().and_then(|j| j.not_before)
    }
}

/// How the worker reaches the server.
pub trait Sink<T>: Send + Sync + 'static {
    fn send(&self, target: &T, op: &Op) -> impl Future<Output = Result<(), String>> + Send;
}

/// Tuning, so tests can run quickly.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Longest a single request may take before it is abandoned.
    pub timeout: Duration,
    /// Attempts made at an important job (the first included).
    pub attempts: u32,
    pub backoff: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self { timeout: Duration::from_secs(10), attempts: 4, backoff: Duration::from_secs(2) }
    }
}

enum Msg<T> {
    Job(T, Op),
    Flush(oneshot::Sender<()>),
}

/// The sending side. Cheap to use from anywhere; never blocks.
pub struct Reporter<T> {
    tx: mpsc::UnboundedSender<Msg<T>>,
}

impl<T: PartialEq + Clone + Send + 'static> Reporter<T> {
    /// Start a worker on `runtime`.
    pub fn spawn<S: Sink<T>>(runtime: &tokio::runtime::Handle, sink: S, config: Config) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        runtime.spawn(worker(rx, sink, config));
        Self { tx }
    }

    pub fn send(&self, target: T, op: Op) {
        let _ = self.tx.send(Msg::Job(target, op));
    }

    /// Wait until everything sent so far was delivered or given up on; false if `timeout` came first.
    pub async fn flush(&self, timeout: Duration) -> bool {
        let (done, wait) = oneshot::channel();
        if self.tx.send(Msg::Flush(done)).is_err() {
            return true;
        }
        tokio::time::timeout(timeout, wait).await.is_ok()
    }
}

async fn worker<T: PartialEq + Clone + Send + 'static, S: Sink<T>>(mut rx: mpsc::UnboundedReceiver<Msg<T>>, sink: S, config: Config) {
    let mut pending = Pending::<T>::default();
    let mut waiters: Vec<oneshot::Sender<()>> = Vec::new();
    let mut closed = false;
    loop {
        // Take in everything that arrived while the last request was running.
        loop {
            match rx.try_recv() {
                Ok(Msg::Job(target, op)) => pending.push(target, op),
                Ok(Msg::Flush(done)) => waiters.push(done),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
            }
        }
        if pending.is_empty() {
            for w in waiters.drain(..) {
                let _ = w.send(());
            }
            if closed {
                return;
            }
            match rx.recv().await {
                Some(Msg::Job(target, op)) => pending.push(target, op),
                Some(Msg::Flush(done)) => waiters.push(done),
                None => return,
            }
            continue;
        }
        // A retried job waits out its backoff, still absorbing new jobs.
        if let Some(at) = pending.next_ready().filter(|at| *at > Instant::now()) {
            if closed {
                tokio::time::sleep_until(at.into()).await;
            } else {
                tokio::select! {
                    _ = tokio::time::sleep_until(at.into()) => {}
                    msg = rx.recv() => match msg {
                        Some(Msg::Job(target, op)) => pending.push(target, op),
                        Some(Msg::Flush(done)) => waiters.push(done),
                        None => closed = true,
                    },
                }
                continue;
            }
        }
        let Some(job) = pending.pop() else { continue };
        let sent = tokio::time::timeout(config.timeout, sink.send(&job.target, &job.op)).await;
        let error = match sent {
            Ok(Ok(())) => continue,
            Ok(Err(e)) => e,
            Err(_) => format!("no answer within {}s", config.timeout.as_secs_f32()),
        };
        if job.op.is_important() && job.attempts + 1 < config.attempts {
            tracing::debug!("plex report failed, will retry: {error}");
            let delay = config.backoff * 2u32.pow(job.attempts);
            pending.retry(job, delay);
        } else {
            tracing::debug!("plex report dropped: {error}");
        }
    }
}

/// The app's sink: talks to the Plex server a job was queued for.
struct PlexSink;

impl Sink<PlexServer> for PlexSink {
    async fn send(&self, server: &PlexServer, op: &Op) -> Result<(), String> {
        let result = match op {
            Op::Timeline(update) => server.timeline(update).await,
            Op::Scrobble { rating_key } => server.scrobble(rating_key).await,
            Op::Streams { part_id, audio, subtitle } => server.select_streams(*part_id, *audio, *subtitle, true).await,
            Op::PlayQueue(QueueOp::Remove { queue, item }) => server.remove_from_play_queue(*queue, *item).await.map(|_| ()),
            Op::PlayQueue(QueueOp::Move { queue, item, after }) => server.move_play_queue_item(*queue, *item, *after).await.map(|_| ()),
            Op::PlayQueue(QueueOp::Shuffle { queue }) => server.shuffle_play_queue(*queue).await.map(|_| ()),
            Op::PlayQueue(QueueOp::Unshuffle { queue }) => server.unshuffle_play_queue(*queue).await.map(|_| ()),
            Op::PingTranscode(session) => server.ping_transcode(session).await,
            Op::StopTranscode(session) => server.stop_transcode(session).await,
        };
        result.map_err(|e| e.to_string())
    }
}

static REPORTER: OnceLock<Reporter<PlexServer>> = OnceLock::new();

/// Wait for the app-wide reporter to deliver what it has (nothing to do if it never started).
pub async fn flush(timeout: Duration) -> bool {
    match REPORTER.get() {
        Some(reporter) => reporter.flush(timeout).await,
        None => true,
    }
}

/// The app-wide reporter. One worker for all playback keeps reports in order
/// across consecutive playbacks (a retry never overtakes the stop before it).
pub fn reporter() -> &'static Reporter<PlexServer> {
    REPORTER.get_or_init(|| Reporter::spawn(&crate::runtime(), PlexSink, Config::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn update(key: &str, state: TimelineState, time_ms: i64) -> Op {
        Op::Timeline(TimelineUpdate {
            rating_key: key.into(),
            key: format!("/library/metadata/{key}"),
            state,
            time_ms,
            duration_ms: 100_000,
            play_queue_item_id: None,
            session: None,
        })
    }

    fn times(p: &Pending<&'static str>) -> Vec<(String, i64, TimelineState)> {
        p.ops()
            .filter_map(|o| match o {
                Op::Timeline(u) => Some((u.rating_key.clone(), u.time_ms, u.state)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn progress_collapses_to_the_newest_checkpoint() {
        let mut p = Pending::default();
        for t in 0..50 {
            p.push("s", update("1", TimelineState::Playing, t * 1000));
        }
        p.push("s", update("1", TimelineState::Paused, 49_500));
        assert_eq!(times(&p), vec![("1".into(), 49_500, TimelineState::Paused)]);
    }

    #[test]
    fn different_items_and_servers_do_not_merge() {
        let mut p = Pending::default();
        p.push("a", update("1", TimelineState::Playing, 1));
        p.push("a", update("2", TimelineState::Playing, 2));
        p.push("b", update("1", TimelineState::Playing, 3));
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn the_final_stop_replaces_progress_and_is_kept() {
        let mut p = Pending::default();
        p.push("s", update("1", TimelineState::Playing, 10_000));
        p.push("s", update("1", TimelineState::Stopped, 90_000));
        p.push("s", Op::Scrobble { rating_key: "1".into() });
        assert_eq!(times(&p), vec![("1".into(), 90_000, TimelineState::Stopped)]);
        assert!(matches!(p.ops().nth(1), Some(Op::Scrobble { .. })), "the scrobble stays behind the stop");
    }

    #[test]
    fn progress_of_a_new_playback_never_merges_into_an_earlier_stop() {
        let mut p = Pending::default();
        p.push("s", update("1", TimelineState::Stopped, 90_000));
        p.push("s", update("1", TimelineState::Playing, 0));
        p.push("s", update("1", TimelineState::Playing, 5_000));
        assert_eq!(
            times(&p),
            vec![("1".into(), 90_000, TimelineState::Stopped), ("1".into(), 5_000, TimelineState::Playing)],
            "the retry's progress must be sent after the stop, and never replace it"
        );
    }

    #[test]
    fn stream_choices_merge_per_part() {
        let mut p = Pending::default();
        p.push("s", Op::Streams { part_id: 7, audio: Some(1), subtitle: None });
        p.push("s", Op::Streams { part_id: 7, audio: None, subtitle: Some(5) });
        p.push("s", Op::Streams { part_id: 7, audio: Some(2), subtitle: None });
        p.push("s", Op::Streams { part_id: 8, audio: Some(9), subtitle: None });
        let ops: Vec<_> = p.ops().cloned().collect();
        assert_eq!(
            ops,
            vec![Op::Streams { part_id: 7, audio: Some(2), subtitle: Some(5) }, Op::Streams { part_id: 8, audio: Some(9), subtitle: None }]
        );
    }

    #[test]
    fn transcode_pings_are_deduplicated_and_dropped_when_stopping() {
        let mut p = Pending::default();
        for _ in 0..5 {
            p.push("s", Op::PingTranscode("x".into()));
        }
        assert_eq!(p.len(), 1);
        p.push("s", Op::StopTranscode("x".into()));
        assert_eq!(p.ops().cloned().collect::<Vec<_>>(), vec![Op::StopTranscode("x".into())]);
        p.push("s", Op::PingTranscode("x".into()));
        assert_eq!(p.len(), 1, "no ping after the stop");
    }

    /// Records what reaches "Plex"; each request takes `delay`, and the first `fail` of them fail.
    #[derive(Clone)]
    struct Fake {
        seen: Arc<Mutex<Vec<Op>>>,
        delay: Duration,
        fail: Arc<Mutex<u32>>,
        hang: bool,
    }

    impl Fake {
        fn new(delay: Duration, fail: u32) -> Self {
            Self { seen: Arc::default(), delay, fail: Arc::new(Mutex::new(fail)), hang: false }
        }
    }

    impl Sink<&'static str> for Fake {
        async fn send(&self, _: &&'static str, op: &Op) -> Result<(), String> {
            if self.hang {
                std::future::pending::<()>().await;
            }
            tokio::time::sleep(self.delay).await;
            {
                let mut fail = self.fail.lock().unwrap();
                if *fail > 0 {
                    *fail -= 1;
                    return Err("boom".into());
                }
            }
            self.seen.lock().unwrap().push(op.clone());
            Ok(())
        }
    }

    fn fast() -> Config {
        Config { timeout: Duration::from_millis(200), attempts: 4, backoff: Duration::from_millis(10) }
    }

    #[tokio::test]
    async fn a_slow_server_costs_the_caller_nothing_and_collapses_the_backlog() {
        let fake = Fake::new(Duration::from_millis(150), 0);
        let reporter = Reporter::spawn(&tokio::runtime::Handle::current(), fake.clone(), fast());
        reporter.send("s", update("1", TimelineState::Playing, 0));
        // The first request is now in flight and takes 150ms.
        tokio::time::sleep(Duration::from_millis(20)).await;
        let began = Instant::now();
        for t in 1..200 {
            reporter.send("s", update("1", TimelineState::Playing, t * 1000));
        }
        reporter.send("s", update("1", TimelineState::Stopped, 200_000));
        reporter.send("s", Op::Scrobble { rating_key: "1".into() });
        assert!(began.elapsed() < Duration::from_millis(50), "sending must not wait for Plex");
        assert!(reporter.flush(Duration::from_secs(5)).await);
        let seen = fake.seen.lock().unwrap().clone();
        assert!(seen.len() <= 4, "200 checkpoints must not become 200 requests: {}", seen.len());
        let stops: Vec<_> = seen.iter().filter(|o| matches!(o, Op::Timeline(u) if u.state == TimelineState::Stopped)).collect();
        assert_eq!(stops.len(), 1);
        assert!(matches!(&seen[seen.len() - 2], Op::Timeline(u) if u.state == TimelineState::Stopped && u.time_ms == 200_000));
        assert!(matches!(seen.last(), Some(Op::Scrobble { .. })));
        // Nothing older than the stop was sent after it.
        let stop_at = seen.iter().position(|o| matches!(o, Op::Timeline(u) if u.state == TimelineState::Stopped)).unwrap();
        assert!(seen[stop_at..].iter().all(|o| !o.is_progress()));
    }

    #[tokio::test]
    async fn the_final_stop_survives_failures() {
        let fake = Fake::new(Duration::from_millis(1), 2);
        let reporter = Reporter::spawn(&tokio::runtime::Handle::current(), fake.clone(), fast());
        reporter.send("s", update("1", TimelineState::Stopped, 42_000));
        assert!(reporter.flush(Duration::from_secs(5)).await);
        let seen = fake.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert!(matches!(&seen[0], Op::Timeline(u) if u.time_ms == 42_000 && u.state == TimelineState::Stopped));
    }

    #[tokio::test]
    async fn failed_progress_is_not_retried_and_not_resent_over_newer_state() {
        let fake = Fake::new(Duration::from_millis(1), 1);
        let reporter = Reporter::spawn(&tokio::runtime::Handle::current(), fake.clone(), fast());
        reporter.send("s", update("1", TimelineState::Playing, 1_000));
        tokio::time::sleep(Duration::from_millis(50)).await;
        reporter.send("s", update("1", TimelineState::Playing, 2_000));
        assert!(reporter.flush(Duration::from_secs(5)).await);
        let seen = fake.seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert!(matches!(&seen[0], Op::Timeline(u) if u.time_ms == 2_000));
    }

    #[tokio::test]
    async fn a_hanging_server_is_cut_off_so_later_reports_get_through() {
        let mut hang = Fake::new(Duration::ZERO, 0);
        hang.hang = true;
        let reporter = Reporter::spawn(&tokio::runtime::Handle::current(), hang, Config { timeout: Duration::from_millis(30), attempts: 2, backoff: Duration::from_millis(5) });
        reporter.send("s", update("1", TimelineState::Stopped, 1));
        reporter.send("s", update("2", TimelineState::Stopped, 2));
        let began = Instant::now();
        assert!(reporter.flush(Duration::from_secs(5)).await);
        assert!(began.elapsed() < Duration::from_secs(2), "{:?}", began.elapsed());
    }

    #[tokio::test]
    async fn flush_times_out_while_the_server_is_slow() {
        let fake = Fake::new(Duration::from_millis(300), 0);
        let reporter = Reporter::spawn(&tokio::runtime::Handle::current(), fake, Config { timeout: Duration::from_secs(5), ..fast() });
        reporter.send("s", update("1", TimelineState::Stopped, 1));
        assert!(!reporter.flush(Duration::from_millis(20)).await);
    }
}
