//! The built-in critic: the `picturesque-critic` helper process, which runs
//! the downloaded model with llama.cpp (ADR-0016, `crates/critic`).
//!
//! The helper is started on the first critique and kept while critiques
//! follow each other, so the model stays loaded; after [`IDLE`] without one
//! it is stopped and its memory returned. If it dies (llama.cpp aborts on
//! some errors), that critique fails and the next one starts a new helper.
//!
//! Background critiques may run it at low priority. A process cannot raise
//! its own priority again, so the helper is restarted when the priority
//! changes (the model reloads from the page cache in a second or two), and
//! [`HelperCritic::interrupt`] stops a background critique by ending the
//! process.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use picturesque_app::ports::critic::{CriticError, Throttle};
use picturesque_core::critic_protocol::{HelperReply, HelperRequest};

/// How long the model stays loaded after a critique.
pub const IDLE: Duration = Duration::from_secs(5 * 60);
/// Longest reply, in tokens; critiques with advice take about 300.
const MAX_TOKENS: u32 = 768;

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
    /// Shared so that [`HelperCritic::interrupt`] can end it while a
    /// critique waits for its reply.
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    last_used: Instant,
    low_priority: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        // It would exit once its stdin closes, but may be mid-critique.
        let mut child = self.child.lock().expect("child lock");
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// The background critique in progress, which may be interrupted.
#[derive(Default)]
struct Current {
    child: Option<Arc<Mutex<Child>>>,
    interrupted: bool,
    /// A background `ask` is under way (thumbnailing, start-up or waiting).
    in_progress: bool,
    /// [`HelperCritic::interrupt`] came before `child` was registered.
    interrupt_requested: bool,
}

/// Marks a background `ask` as in progress while it lives.
struct InProgress<'a>(&'a Mutex<Current>);

impl<'a> InProgress<'a> {
    fn new(current: &'a Mutex<Current>) -> Self {
        let mut c = current.lock().expect("current lock");
        c.in_progress = true;
        c.interrupt_requested = false;
        Self(current)
    }
}

impl Drop for InProgress<'_> {
    fn drop(&mut self) {
        *self.0.lock().expect("current lock") = Current::default();
    }
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    /// Whether a thread is watching for idleness.
    watching: bool,
}

/// A test's hook into `ask`.
#[cfg(test)]
type Hook = Box<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct HelperCritic {
    helper: Option<PathBuf>,
    idle: Duration,
    state: Arc<Mutex<State>>,
    current: Arc<Mutex<Current>>,
    /// Called once the helper is unregistered, before `ask` returns.
    #[cfg(test)]
    after_reply: Arc<Mutex<Option<Hook>>>,
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
            current: Arc::default(),
            #[cfg(test)]
            after_reply: Arc::default(),
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

    /// Whether the running helper has low priority.
    pub fn is_low_priority(&self) -> bool {
        self.state
            .lock()
            .expect("helper lock")
            .running
            .as_ref()
            .is_some_and(|r| r.low_priority)
    }

    /// Ends the helper if it is writing a background critique.
    pub fn interrupt(&self) {
        let mut current = self.current.lock().expect("current lock");
        if let Some(child) = current.child.clone() {
            current.interrupted = true;
            let _ = child.lock().expect("child lock").kill();
        } else if current.in_progress {
            // Not started yet: `ask` stops before sending the request.
            current.interrupt_requested = true;
        }
    }

    /// Asks the helper; `request.max_tokens` and `threads` are set here.
    /// `background` critiques may be interrupted.
    pub fn ask(
        &self,
        mut request: HelperRequest,
        background: Option<Throttle>,
    ) -> Result<String, CriticError> {
        if let Some(problem) = self.problem() {
            return Err(CriticError::Unreachable(problem));
        }
        request.max_tokens = MAX_TOKENS;
        request.threads = background.and_then(|t| t.threads).unwrap_or(0);
        let low_priority = background.is_some_and(|t| t.low_priority);
        let line = serde_json::to_string(&request).expect("requests serialize");
        let _in_progress = background.map(|_| InProgress::new(&self.current));
        let mut state = self.state.lock().expect("helper lock");
        // A user's critique between background ones changes the priority,
        // so the helper restarts and reloads the model (1-2 s); by design.
        if state
            .running
            .as_ref()
            .is_some_and(|r| r.low_priority != low_priority)
        {
            state.running = None;
        }
        if state.running.is_none() {
            state.running = Some(self.start(low_priority)?);
        }
        if !state.watching {
            state.watching = true;
            watch(Arc::downgrade(&self.state), self.idle);
        }
        let running = state.running.as_mut().expect("just started");
        if background.is_some() {
            let mut current = self.current.lock().expect("current lock");
            if std::mem::take(&mut current.interrupt_requested) {
                return Err(CriticError::Interrupted);
            }
            current.child = Some(Arc::clone(&running.child));
        }
        let reply = exchange(running, &line);
        // Unregister the helper and read `interrupted` under one lock: an
        // `interrupt` after this finds nothing to kill, so it cannot end the
        // helper the next critique uses.
        let interrupted = {
            let mut current = self.current.lock().expect("current lock");
            current.child = None;
            current.interrupted
        };
        #[cfg(test)]
        if let Some(hook) = self.after_reply.lock().expect("hook lock").as_ref() {
            hook();
        }
        if interrupted {
            state.running = None;
            return Err(CriticError::Interrupted);
        }
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
                let dead = state.running.take().expect("running");
                let status = exit_status(&mut dead.child.lock().expect("child lock"));
                drop(dead);
                Err(CriticError::Server(match status {
                    Some(status) => format!("the built-in critic stopped ({status}): {e}"),
                    None => format!("the built-in critic stopped answering: {e}"),
                }))
            }
        }
    }

    fn start(&self, low_priority: bool) -> Result<Running, CriticError> {
        let path = self.helper.as_deref().expect("checked by problem()");
        let mut child = command(path, low_priority)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // llama.cpp's own messages, and the reason if it aborts.
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| CriticError::Unreachable(start_error(path, &e)))?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        Ok(Running {
            child: Arc::new(Mutex::new(child)),
            stdin,
            stdout,
            last_used: Instant::now(),
            low_priority,
        })
    }
}

/// The command starting the helper, at low priority if asked: through
/// `nice` on Unix (it replaces itself with the helper), with a lower
/// priority class on Windows.
fn command(path: &Path, low_priority: bool) -> Command {
    if !low_priority {
        return Command::new(path);
    }
    #[cfg(unix)]
    {
        let mut nice = Command::new("nice");
        nice.arg("-n").arg("10").arg("--").arg(path);
        nice
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        let mut command = Command::new(path);
        command.creation_flags(BELOW_NORMAL_PRIORITY_CLASS);
        command
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Serializes tests that write and run fake helpers (*Text file busy*).
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A fake helper logging each start to `dir/starts` and each request to
    /// `dir/requests`, answering every request.
    fn fake(dir: &Path) -> PathBuf {
        let path = dir.join("picturesque-critic");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\necho start >> '{}'\nwhile IFS= read -r line; do\necho req >> '{}'\n\
                 echo '{{\"status\":\"done\",\"text\":\"nice\",\"device\":\"CPU\",\"seconds\":0.1}}'\ndone\n",
                dir.join("starts").display(),
                dir.join("requests").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn lines(dir: &Path, name: &str) -> usize {
        std::fs::read_to_string(dir.join(name)).map_or(0, |s| s.lines().count())
    }

    fn request() -> HelperRequest {
        HelperRequest {
            model: "m.gguf".into(),
            mmproj: "p.gguf".into(),
            gpu: false,
            threads: 0,
            system: String::new(),
            prompt: String::new(),
            image: String::new(),
            schema: "{}".into(),
            max_tokens: 0,
        }
    }

    const BACKGROUND: Throttle = Throttle {
        low_priority: false,
        threads: None,
    };

    #[test]
    fn an_interrupt_during_start_up_cancels_the_background_critique() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let critic = HelperCritic::with_helper(Some(fake(dir.path())), Duration::from_secs(60));
        // `ask` marks itself in progress, then blocks on the state lock.
        let held = critic.state.lock().unwrap();
        let asking = critic.clone();
        let thread = std::thread::spawn(move || asking.ask(request(), Some(BACKGROUND)));
        while !critic.current.lock().unwrap().in_progress {
            std::thread::sleep(Duration::from_millis(1));
        }
        critic.interrupt();
        drop(held);
        assert!(matches!(
            thread.join().unwrap(),
            Err(CriticError::Interrupted)
        ));
        assert_eq!(lines(dir.path(), "requests"), 0);
    }

    #[test]
    fn an_interrupt_after_the_reply_does_not_end_the_helper() {
        let _serial = serial();
        let dir = tempfile::tempdir().unwrap();
        let critic = HelperCritic::with_helper(Some(fake(dir.path())), Duration::from_secs(60));
        let interrupting = critic.clone();
        *critic.after_reply.lock().unwrap() = Some(Box::new(move || interrupting.interrupt()));
        // The interrupt lands after the reply is in and the helper is
        // unregistered: too late to matter.
        assert_eq!(critic.ask(request(), Some(BACKGROUND)).unwrap(), "nice");
        *critic.after_reply.lock().unwrap() = None;
        // The helper lives on for the next critique.
        assert_eq!(critic.ask(request(), Some(BACKGROUND)).unwrap(), "nice");
        assert_eq!(lines(dir.path(), "starts"), 1);
    }
}
