//! The built-in critic: the `picturesque-critic` helper process, which runs
//! the downloaded model with llama.cpp (ADR-0016, `crates/critic`).
//!
//! The helper is started on the first critique and kept while critiques
//! follow each other, so the model stays loaded; after [`IDLE`] without one
//! it is stopped and its memory returned. If it dies (llama.cpp aborts on
//! some errors), that critique fails and the next one starts a new helper.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use picturesque_app::ports::critic::CriticError;
use picturesque_core::critic_protocol::{HelperReply, HelperRequest};

/// How long the model stays loaded after a critique.
pub const IDLE: Duration = Duration::from_secs(5 * 60);
/// Longest reply, in tokens; critiques take about 120.
const MAX_TOKENS: u32 = 512;

const NAME: &str = "picturesque-critic";

/// `PICTURESQUE_CRITIC_HELPER`, or the helper in its `critic` folder
/// (`scripts/critic-libs.sh`) beside the executable: next to it
/// (development builds, Windows, macOS bundles), one folder up (tests and
/// examples in `target/<profile>/deps` and `examples`), or in
/// `../lib/picturesque` (deb, rpm and AppImage).
pub fn locate() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PICTURESQUE_CRITIC_HELPER").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = format!("{NAME}{}", std::env::consts::EXE_SUFFIX);
    [
        dir.join("critic"),
        dir.join("../critic"),
        dir.join("../lib/picturesque/critic"),
    ]
    .map(|folder| folder.join(&name))
    .into_iter()
    .find(|p| p.is_file())
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    last_used: Instant,
}

impl Drop for Running {
    fn drop(&mut self) {
        // It would exit once its stdin closes, but may be mid-critique.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    /// Whether a thread is watching for idleness.
    watching: bool,
}

#[derive(Clone)]
pub struct HelperCritic {
    helper: Option<PathBuf>,
    idle: Duration,
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for HelperCritic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelperCritic")
            .field("helper", &self.helper)
            .finish_non_exhaustive()
    }
}

impl HelperCritic {
    /// The helper found by [`locate`].
    pub fn new() -> Self {
        Self::with_helper(locate(), IDLE)
    }

    /// `helper` (none: the built-in critic is not installed), stopped after
    /// `idle`.
    pub fn with_helper(helper: Option<PathBuf>, idle: Duration) -> Self {
        Self {
            helper,
            idle,
            state: Arc::default(),
        }
    }

    /// Why it cannot run, or `None`.
    pub fn problem(&self) -> Option<String> {
        match &self.helper {
            Some(path) if path.is_file() => None,
            _ => Some(format!(
                "The built-in critic is not part of this installation ({NAME} was not \
                 found next to the app). Reinstall Picturesque, or use a local server."
            )),
        }
    }

    /// Whether a helper process is running.
    pub fn is_running(&self) -> bool {
        self.state.lock().expect("helper lock").running.is_some()
    }

    /// Asks the helper; `request.max_tokens` is set here.
    pub fn ask(&self, mut request: HelperRequest) -> Result<String, CriticError> {
        if let Some(problem) = self.problem() {
            return Err(CriticError::Unreachable(problem));
        }
        request.max_tokens = MAX_TOKENS;
        let line = serde_json::to_string(&request).expect("requests serialize");
        let mut state = self.state.lock().expect("helper lock");
        if state.running.is_none() {
            state.running = Some(self.start()?);
        }
        if !state.watching {
            state.watching = true;
            watch(Arc::downgrade(&self.state), self.idle);
        }
        let running = state.running.as_mut().expect("just started");
        let reply = exchange(running, &line);
        match reply {
            Ok(reply) => {
                running.last_used = Instant::now();
                match reply {
                    HelperReply::Done { text, .. } => Ok(text),
                    HelperReply::Failed { message } => Err(CriticError::Server(message)),
                }
            }
            Err(e) => {
                // Dead or confused: start afresh next time.
                let mut dead = state.running.take().expect("running");
                let status = exit_status(&mut dead.child);
                drop(dead);
                Err(CriticError::Server(match status {
                    Some(status) => format!("the built-in critic stopped ({status}): {e}"),
                    None => format!("the built-in critic stopped answering: {e}"),
                }))
            }
        }
    }

    fn start(&self) -> Result<Running, CriticError> {
        let path = self.helper.as_deref().expect("checked by problem()");
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // llama.cpp's own messages, and the reason if it aborts.
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| CriticError::Unreachable(start_error(path, &e)))?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        Ok(Running {
            child,
            stdin,
            stdout,
            last_used: Instant::now(),
        })
    }
}

impl Default for HelperCritic {
    fn default() -> Self {
        Self::new()
    }
}

fn start_error(path: &Path, e: &std::io::Error) -> String {
    format!("cannot start {}: {e}", path.display())
}

/// How the helper ended, waiting a moment for a process that just closed
/// its output; `None` if it is still running.
fn exit_status(child: &mut Child) -> Option<std::process::ExitStatus> {
    for _ in 0..20 {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    None
}

/// Sends one request line and reads one reply line.
fn exchange(running: &mut Running, line: &str) -> Result<HelperReply, String> {
    writeln!(running.stdin, "{line}")
        .and_then(|()| running.stdin.flush())
        .map_err(|e| e.to_string())?;
    let mut reply = String::new();
    let n = running
        .stdout
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("it closed its output".into());
    }
    serde_json::from_str(&reply).map_err(|e| format!("unreadable reply: {e}"))
}

/// Stops the helper once it has been idle for `idle`; ends with the critic
/// or when no helper runs.
fn watch(state: Weak<Mutex<State>>, idle: Duration) {
    let tick = idle.min(Duration::from_secs(30)) / 2;
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(tick);
            let Some(state) = state.upgrade() else { return };
            let mut state = state.lock().expect("helper lock");
            match &state.running {
                Some(r) if r.last_used.elapsed() >= idle => state.running = None,
                Some(_) => continue,
                None => {
                    state.watching = false;
                    return;
                }
            }
        }
    });
}
