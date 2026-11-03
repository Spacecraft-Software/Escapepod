// SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The single-thread `poll(2)` loop that drives the daemon, the status
//! socket, `daemon status` and `event monitor`.
//!
//! Serial by design (manual, `Concurrency`): one thread polls the
//! keyboards, the pointers, an inotify watch on the device directory, a
//! `signalfd` and the status socket, with no timeout while idle.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use escapepod_core::{DeviceId, Engine, Input, Instant, KeyEvent};
use escapepod_evdev::{DeviceKind, EvdevOpener, Keyboard, Opener, UinputKeyboard};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use nix::sys::signal::{SigSet, Signal};
use nix::sys::signalfd::{SfdFlags, SignalFd};
use nix::unistd::{Gid, Group};
use serde_json::Value;

use crate::app::{self, Io};
use crate::cli::BackendArg;
use crate::config::Config;
use crate::daemon::{Daemon, Flow};
use crate::error::{AppError, Exit};
use crate::monitor::{self, Record};
use crate::output::{Env, Format, Mode, Severity};

/// The service's socket directory, created by `tmpfiles.d`.
pub const SERVICE_SOCKET_DIR: &str = "/run/escapepod";

/// The group allowed to read the status socket (ESC-SRS-059).
pub const CONTROL_GROUP: &str = "escapepod-ctl";

/// The status socket for this process: `ESCAPEPOD_SOCKET`, else the
/// service's when its directory is writable, else the session's under
/// `XDG_RUNTIME_DIR`.
pub fn socket_path(env: &Env) -> Option<PathBuf> {
    if let Some(p) = env.var("ESCAPEPOD_SOCKET") {
        return Some(PathBuf::from(p));
    }
    let service = Path::new(SERVICE_SOCKET_DIR);
    if nix::unistd::access(service, nix::unistd::AccessFlags::W_OK).is_ok() {
        return Some(service.join("escapepod.sock"));
    }
    env.var("XDG_RUNTIME_DIR")
        .map(|d| Path::new(d).join("escapepod.sock"))
}

/// Where `daemon status` looks: `--socket`, `ESCAPEPOD_SOCKET`, the
/// service's socket when it exists, else the session's.
fn client_socket(explicit: Option<&Path>, env: &Env) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(p) = env.var("ESCAPEPOD_SOCKET") {
        return Some(PathBuf::from(p));
    }
    let service = Path::new(SERVICE_SOCKET_DIR).join("escapepod.sock");
    if service.exists() {
        return Some(service);
    }
    env.var("XDG_RUNTIME_DIR")
        .map(|d| Path::new(d).join("escapepod.sock"))
}

/// Binds the status socket at `path`: mode `0660` and group `gid` when the
/// control group exists, else `0600`. A live socket at `path` means
/// another daemon runs (`AddrInUse`); a stale one is replaced.
/// Implements: ESC-SRS-059
///
/// # Errors
///
/// `AddrInUse` for a live socket, or the bind, chmod or chown error.
pub fn prepare_socket(path: &Path, gid: Option<Gid>) -> io::Result<UnixListener> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::from(io::ErrorKind::AddrInUse));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    let mode = if gid.is_some() { 0o660 } else { 0o600 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    if let Some(g) = gid {
        nix::unistd::chown(path, None, Some(g)).map_err(io::Error::from)?;
    }
    listener.set_nonblocking(true)?;
    Ok(listener)
}

struct Client {
    stream: UnixStream,
    subscribed: bool,
}

/// The connected status clients.
#[derive(Default)]
struct Clients(Vec<Client>);

impl Clients {
    /// Accepts every waiting connection and sends each one status line.
    fn accept(&mut self, listener: &UnixListener, status: &Value) {
        while let Ok((stream, _)) = listener.accept() {
            let _ = stream.set_nonblocking(true);
            let mut c = Client {
                stream,
                subscribed: false,
            };
            if c.send(status) {
                self.0.push(c);
            }
        }
    }

    /// Reads requests from client `i`: `events` subscribes it to the
    /// record stream; end of file drops it.
    fn readable(&mut self, i: usize) {
        let Some(c) = self.0.get_mut(i) else { return };
        let mut buf = [0u8; 256];
        match c.stream.read(&mut buf) {
            Ok(0) | Err(_) => {
                self.0.remove(i);
            }
            Ok(n) => {
                if String::from_utf8_lossy(&buf[..n]).contains("events") {
                    c.subscribed = true;
                }
            }
        }
    }

    /// Sends redacted records to subscribers, dropping any that fail.
    fn publish(&mut self, records: &[Record]) {
        if records.is_empty() {
            return;
        }
        self.0
            .retain_mut(|c| !c.subscribed || records.iter().all(|r| c.send(&r.to_json())));
    }
}

impl Client {
    fn send(&mut self, v: &Value) -> bool {
        let mut line = v.to_string();
        line.push('\n');
        self.stream.write_all(line.as_bytes()).is_ok()
    }
}

fn now(start: std::time::Instant) -> Instant {
    Instant::from_micros(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX))
}

/// Blocks the termination and reload signals and returns a descriptor
/// they arrive on.
fn signals() -> Result<SignalFd, AppError> {
    let mut mask = SigSet::empty();
    for s in [Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP] {
        mask.add(s);
    }
    mask.thread_block()
        .and_then(|()| SignalFd::with_flags(&mask, SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC))
        .map_err(|e| {
            AppError::new(
                "INTERNAL_ERROR",
                Exit::Failure,
                format!("cannot set up signal handling: {e}"),
                "escapepod daemon run --dry-run",
            )
        })
}

fn watch(dev_dir: &Path) -> Option<Inotify> {
    let i = Inotify::init(InitFlags::IN_NONBLOCK | InitFlags::IN_CLOEXEC).ok()?;
    i.add_watch(dev_dir, AddWatchFlags::IN_CREATE | AddWatchFlags::IN_ATTRIB)
        .ok()?;
    Some(i)
}

/// What one `poll(2)` wake-up found ready.
enum Ready {
    Signal,
    Hotplug,
    Listener,
    Client(usize),
    Keyboard(DeviceId),
    Pointer(DeviceId),
}

/// `daemon run` on the evdev backend, after its guards.
///
/// # Errors
///
/// `BACKEND_UNAVAILABLE` (exit 4) when the virtual keyboard cannot be
/// created, before any device is opened; `CONFLICT` (exit 5) when another
/// daemon holds the socket.
pub fn run(
    cfg: Config,
    backend: BackendArg,
    only_device: Option<(u16, u16)>,
    mode: &Mode,
    io: &mut Io<'_>,
    reload: &dyn Fn() -> Result<Config, AppError>,
) -> Result<Exit, AppError> {
    let probe = io.env.var("ESCAPEPOD_UINPUT").unwrap_or("/dev/uinput");
    let uinput = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(probe)
        .map(drop);
    match select(
        backend,
        uinput.is_ok(),
        || any_keyboard_readable(&cfg, io.env),
        io.env.var("DISPLAY").is_some(),
    ) {
        Selected::X11 => {
            if cfg.has_buttons() {
                return Err(AppError::invalid(
                    "the X11 backend cannot suppress mouse buttons",
                    "escapepod daemon run --backend evdev --dry-run",
                ));
            }
            if only_device.is_some() {
                return Err(AppError::invalid(
                    "--only-device requires the evdev backend",
                    "escapepod daemon run --backend evdev --only-device 1234:5678 --dry-run",
                ));
            }
            if backend == BackendArg::Auto {
                mode.diag(
                    io.err,
                    Severity::Info,
                    "BACKEND_SELECTED",
                    "no readable keyboard or no /dev/uinput; using the X11 backend",
                    None,
                );
            }
            return crate::x11::run(&cfg, mode, io);
        }
        Selected::None => {
            return Err(match uinput {
                Err(e) if backend == BackendArg::Evdev => no_uinput(&e),
                _ => AppError::new(
                    "BACKEND_UNAVAILABLE",
                    Exit::Permission,
                    "no backend: no readable keyboard with /dev/uinput, and DISPLAY is not set; no device was grabbed",
                    "human: run as the escapepod service account (manual: Service), or in an X session with --backend x11",
                ),
            });
        }
        Selected::Evdev => {}
    }
    with_outputs(
        UinputKeyboard::create,
        UinputKeyboard::enable_pointer,
        |sink| {
            evdev_loop(
                Daemon::new(EvdevOpener, sink, cfg).restrict_devices(only_device),
                mode,
                io,
                reload,
            )
        },
    )
}

// Implements: ESC-SRS-109
fn with_outputs<S>(
    keyboard: impl FnOnce() -> io::Result<S>,
    pointer: impl FnOnce(&mut S) -> io::Result<()>,
    start: impl FnOnce(S) -> Result<Exit, AppError>,
) -> Result<Exit, AppError> {
    let mut sink = keyboard().map_err(|e| no_uinput(&e))?;
    pointer(&mut sink).map_err(|e| {
        AppError::new(
            "BACKEND_UNAVAILABLE",
            Exit::Permission,
            format!("cannot create the virtual pointer: {e}; no device was grabbed"),
            "human: give the escapepod account write access to /dev/uinput (manual: Service)",
        )
    })?;
    start(sink)
}

/// The backend `daemon run` uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selected {
    /// The kernel input layer.
    Evdev,
    /// The X11 fallback.
    X11,
    /// Neither is available: exit 4.
    None,
}

/// Chooses the backend. `auto` takes evdev when `/dev/uinput` is writable
/// and a matched keyboard is readable, else X11 when `DISPLAY` is set,
/// else none. A forced backend is used or fails; it never falls back.
/// Implements: ESC-SRS-063, ESC-SRS-064, ESC-SRS-100
pub fn select(
    backend: BackendArg,
    uinput: bool,
    keyboard: impl FnOnce() -> bool,
    display: bool,
) -> Selected {
    match backend {
        BackendArg::Evdev if uinput => Selected::Evdev,
        BackendArg::X11 => Selected::X11,
        BackendArg::Auto if uinput && keyboard() => Selected::Evdev,
        BackendArg::Auto if display => Selected::X11,
        BackendArg::Evdev | BackendArg::Auto => Selected::None,
    }
}

/// Whether any keyboard the configuration matches can be opened for
/// reading (opened and closed at once; never grabbed).
fn any_keyboard_readable(cfg: &Config, env: &Env) -> bool {
    app::scan_devices(env).unwrap_or_default().iter().any(|d| {
        (d.kind == DeviceKind::Keyboard || d.kind == DeviceKind::Pointer && cfg.has_buttons())
            && !d.is_own()
            && cfg.rules_for(d).is_some()
            && std::fs::File::open(&d.path).is_ok()
    })
}

/// The error for a virtual keyboard that cannot be created
/// (ESC-SRS-043): exit 4, and nothing has been grabbed.
pub fn no_uinput(e: &io::Error) -> AppError {
    AppError::new(
        "BACKEND_UNAVAILABLE",
        Exit::Permission,
        format!("cannot create the virtual keyboard on /dev/uinput: {e}; no device was grabbed"),
        "human: give the escapepod account write access to /dev/uinput (manual: Service), or run under X11 with --backend x11",
    )
}

fn evdev_loop(
    mut d: Daemon<EvdevOpener, UinputKeyboard>,
    mode: &Mode,
    io: &mut Io<'_>,
    reload: &dyn Fn() -> Result<Config, AppError>,
) -> Result<Exit, AppError> {
    let start = std::time::Instant::now();
    let sigfd = signals()?;
    let (sysfs, dev_dir) = app::device_roots(io.env);
    let inotify = watch(&dev_dir);
    let gid = Group::from_name(CONTROL_GROUP)
        .ok()
        .flatten()
        .map(|g| g.gid);
    let socket = socket_path(io.env);
    let listener = match socket.as_deref().map(|p| (p, prepare_socket(p, gid))) {
        Some((_, Ok(l))) => Some(l),
        Some((p, Err(e))) if e.kind() == io::ErrorKind::AddrInUse => {
            return Err(AppError::new(
                "CONFLICT",
                Exit::Conflict,
                format!("another daemon is running on {}", p.display()),
                "escapepod daemon status --json",
            ));
        }
        Some((p, Err(e))) => {
            mode.diag(
                io.err,
                Severity::Warn,
                "SOCKET_UNAVAILABLE",
                &format!("no status socket at {}: {e}", p.display()),
                None,
            );
            None
        }
        None => None,
    };
    if listener.is_some() && gid.is_none() {
        mode.diag(
            io.err,
            Severity::Info,
            "SOCKET_PRIVATE",
            "group escapepod-ctl does not exist; the status socket is owner-only (0600)",
            None,
        );
    }
    for info in escapepod_evdev::scan(&sysfs, &dev_dir).unwrap_or_default() {
        d.add(&info, now(start));
    }
    let mut clients = Clients::default();
    loop {
        for n in d.take_notes() {
            mode.diag(io.err, n.severity, n.code, &n.message, None);
        }
        let ready = wait(&d, &sigfd, inotify.as_ref(), listener.as_ref(), &clients)?;
        for r in ready {
            match r {
                Ready::Signal => {
                    while let Ok(Some(info)) = sigfd.read_signal() {
                        if info.ssi_signo == Signal::SIGHUP as u32 {
                            let devices =
                                escapepod_evdev::scan(&sysfs, &dev_dir).unwrap_or_default();
                            d.reload(reload(), &devices, now(start));
                        } else {
                            d.terminate();
                            for n in d.take_notes() {
                                mode.diag(io.err, n.severity, n.code, &n.message, None);
                            }
                            return Ok(Exit::Success);
                        }
                    }
                }
                Ready::Hotplug => {
                    let names: Vec<String> = inotify
                        .as_ref()
                        .and_then(|i| i.read_events().ok())
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|e| e.name.map(|n| n.to_string_lossy().into_owned()))
                        .collect();
                    for name in names {
                        if let Some(info) = escapepod_evdev::info(&sysfs, &dev_dir, &name) {
                            d.hotplug(&info, now(start));
                        }
                    }
                }
                Ready::Listener => {
                    if let Some(l) = &listener {
                        clients.accept(l, &d.status());
                    }
                }
                Ready::Client(i) => clients.readable(i),
                Ready::Keyboard(id) => {
                    if d.keyboard_ready(id, now(start)) == Flow::Emergency {
                        for n in d.take_notes() {
                            mode.diag(io.err, n.severity, n.code, &n.message, None);
                        }
                        return Ok(Exit::Emergency);
                    }
                }
                Ready::Pointer(id) => d.pointer_ready(id, now(start)),
            }
        }
        d.tick(now(start));
        clients.publish(&d.take_records());
    }
}

/// Polls every descriptor once and lists what is ready. The timeout is
/// none while idle (ESC-SRS-057). Implements: ESC-SRS-057
fn wait(
    d: &Daemon<EvdevOpener, UinputKeyboard>,
    sigfd: &SignalFd,
    inotify: Option<&Inotify>,
    listener: Option<&UnixListener>,
    clients: &Clients,
) -> Result<Vec<Ready>, AppError> {
    let mut tags: Vec<Ready> = Vec::new();
    let mut fds: Vec<BorrowedFd<'_>> = Vec::new();
    tags.push(Ready::Signal);
    fds.push(sigfd.as_fd());
    if let Some(i) = inotify {
        tags.push(Ready::Hotplug);
        fds.push(i.as_fd());
    }
    if let Some(l) = listener {
        tags.push(Ready::Listener);
        fds.push(l.as_fd());
    }
    for (i, c) in clients.0.iter().enumerate() {
        tags.push(Ready::Client(i));
        fds.push(c.stream.as_fd());
    }
    for (id, k) in d.keyboards() {
        tags.push(Ready::Keyboard(id));
        fds.push(k.as_fd());
    }
    for (id, p) in d.pointers() {
        tags.push(Ready::Pointer(id));
        fds.push(p.as_fd());
    }
    let mut pfds: Vec<PollFd<'_>> = fds
        .iter()
        .map(|f| PollFd::new(*f, PollFlags::POLLIN))
        .collect();
    let timeout = d.timeout().map_or(PollTimeout::NONE, |t| {
        PollTimeout::try_from(t).unwrap_or(PollTimeout::MAX)
    });
    match poll(&mut pfds, timeout) {
        Ok(_) | Err(nix::errno::Errno::EINTR) => {}
        Err(e) => {
            return Err(AppError::new(
                "INTERNAL_ERROR",
                Exit::Failure,
                format!("poll failed: {e}"),
                "escapepod daemon run --dry-run",
            ));
        }
    }
    let ready: Vec<bool> = pfds
        .iter()
        .map(|p| p.revents().is_some_and(|r| !r.is_empty()))
        .collect();
    // Clients are listed last-first so removing one keeps earlier indices.
    let mut out: Vec<Ready> = tags
        .into_iter()
        .zip(ready)
        .filter_map(|(t, r)| r.then_some(t))
        .collect();
    out.sort_by_key(|r| match r {
        Ready::Client(i) => std::cmp::Reverse(*i),
        _ => std::cmp::Reverse(usize::MAX),
    });
    Ok(out)
}

/// `daemon status`: connect, read the daemon's one status line.
///
/// # Errors
///
/// `NOT_FOUND` (exit 3) when no daemon listens; `PERMISSION_DENIED`
/// (exit 4) without access to the socket.
pub fn status(socket: Option<&Path>, mode: &Mode, io: &mut Io<'_>) -> Result<Exit, AppError> {
    let Some(path) = client_socket(socket, io.env) else {
        return Err(AppError::new(
            "NOT_FOUND",
            Exit::NotFound,
            "no status socket: XDG_RUNTIME_DIR is not set",
            "escapepod daemon status --socket <path>",
        ));
    };
    let stream = UnixStream::connect(&path).map_err(|e| match e.kind() {
        io::ErrorKind::PermissionDenied => AppError::new(
            "PERMISSION_DENIED",
            Exit::Permission,
            format!("no access to {}", path.display()),
            "human: ask an administrator to add your account to the escapepod-ctl group",
        ),
        _ => AppError::new(
            "NOT_FOUND",
            Exit::NotFound,
            format!("no daemon is listening on {}", path.display()),
            "escapepod daemon run --dry-run",
        ),
    })?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).map_err(|e| {
        AppError::new(
            "INTERNAL_ERROR",
            Exit::Failure,
            format!("cannot read {}: {e}", path.display()),
            "escapepod daemon status --json",
        )
    })?;
    let data: Value = serde_json::from_str(line.trim()).map_err(|e| {
        AppError::new(
            "INTERNAL_ERROR",
            Exit::Failure,
            format!("bad status line from {}: {e}", path.display()),
            "escapepod daemon status --json",
        )
    })?;
    mode.emit(io.out, data, |v, _| {
        let mut s = format!(
            "backend: {}\nheld keys: {}\n",
            v["backend"].as_str().unwrap_or("?"),
            v["held"]
        );
        for d in v["devices"].as_array().into_iter().flatten() {
            let _ = writeln!(
                s,
                "{}  {}  {}",
                if d["grabbed"] == true {
                    "grabbed"
                } else {
                    "waiting"
                },
                d["path"].as_str().unwrap_or(""),
                d["name"].as_str().unwrap_or("")
            );
        }
        s
    })?;
    Ok(Exit::Success)
}

/// `event monitor`'s device logic: keyboards read without a grab, one
/// engine each to show the decision the daemon would make.
pub struct Monitor<O: Opener> {
    keyboards: BTreeMap<DeviceId, (String, O::Keyboard, Engine)>,
    buf: Vec<KeyEvent>,
    scratch: Vec<KeyEvent>,
}

impl<O: Opener> Monitor<O> {
    /// Opens every keyboard `cfg` matches, never Escape Pod's own.
    pub fn open(opener: &mut O, cfg: &Config, devices: &[escapepod_evdev::DeviceInfo]) -> Self {
        let mut keyboards = BTreeMap::new();
        for (n, d) in devices
            .iter()
            .filter(|d| d.kind == DeviceKind::Keyboard && !d.is_own())
            .enumerate()
        {
            let Some(rules) = cfg.rules_for(d) else {
                continue;
            };
            if let Ok(k) = opener.keyboard(&d.path) {
                let name = d
                    .path
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                keyboards.insert(
                    DeviceId(u32::try_from(n).unwrap_or(u32::MAX)),
                    (name, k, Engine::new(rules.clone())),
                );
            }
        }
        Self {
            keyboards,
            buf: Vec::new(),
            scratch: Vec::new(),
        }
    }

    /// Whether any keyboard is open.
    pub fn is_empty(&self) -> bool {
        self.keyboards.is_empty()
    }

    /// Reads keyboard `id` and returns one record per key event.
    pub fn ready(&mut self, id: DeviceId, now: Instant, all_keys: bool) -> Vec<Record> {
        let Some((name, k, engine)) = self.keyboards.get_mut(&id) else {
            return Vec::new();
        };
        self.buf.clear();
        if k.read(&mut self.buf).is_err() {
            self.keyboards.remove(&id);
            return Vec::new();
        }
        self.buf
            .iter()
            .map(|&ev| {
                self.scratch.clear();
                let d = engine.step(Input::Key(ev), now, &mut self.scratch);
                monitor::record(
                    engine.rules(),
                    name,
                    ev,
                    d.as_ref(),
                    all_keys,
                    crate::output::now_utc(),
                )
            })
            .collect()
    }

    /// The keyboards, for the poll loop.
    pub fn keyboards(&self) -> impl Iterator<Item = (DeviceId, &O::Keyboard)> {
        self.keyboards.iter().map(|(&id, (_, k, _))| (id, k))
    }
}

/// `event monitor`. Implements: ESC-SRS-062
///
/// # Errors
///
/// `PERMISSION_DENIED` (exit 4) for `--all-keys` off a terminal or under
/// an agent, or when no keyboard can be read.
pub fn monitor(
    cfg: &Config,
    all_keys: bool,
    mode: &Mode,
    io: &mut Io<'_>,
) -> Result<Exit, AppError> {
    if all_keys && (!io.env.stdout_tty || io.env.is_agent()) {
        return Err(AppError::new(
            "PERMISSION_DENIED",
            Exit::Permission,
            "event monitor --all-keys shows every keystroke, so it runs only on a terminal and never under an agent",
            "escapepod event monitor",
        ));
    }
    let devices = app::scan_devices(io.env)?;
    let mut opener = EvdevOpener;
    let mut m = Monitor::open(&mut opener, cfg, &devices);
    if m.is_empty() {
        return Err(AppError::new(
            "PERMISSION_DENIED",
            Exit::Permission,
            "no matched keyboard can be read",
            "human: run event monitor from an account allowed to read /dev/input, or use escapepod daemon status",
        ));
    }
    let sigfd = signals()?;
    let start = std::time::Instant::now();
    loop {
        let ids: Vec<(bool, Option<DeviceId>)> = {
            let mut tags = vec![None];
            let mut fds = vec![sigfd.as_fd()];
            for (id, k) in m.keyboards() {
                tags.push(Some(id));
                fds.push(k.as_fd());
            }
            let mut pfds: Vec<PollFd<'_>> = fds
                .iter()
                .map(|f| PollFd::new(*f, PollFlags::POLLIN))
                .collect();
            let _ = poll(&mut pfds, PollTimeout::NONE);
            tags.into_iter()
                .zip(
                    pfds.iter()
                        .map(|p| p.revents().is_some_and(|r| !r.is_empty())),
                )
                .map(|(t, r)| (r, t))
                .collect()
        };
        for (ready, tag) in ids {
            match (ready, tag) {
                (true, None) => return Ok(Exit::Success),
                (true, Some(id)) => {
                    for r in m.ready(id, now(start), all_keys) {
                        let text = if mode.format == Format::Human {
                            r.line(mode)
                        } else {
                            format!("{}\n", r.to_json())
                        };
                        crate::output::write_all(io.out, text.as_bytes())?;
                    }
                }
                _ => {}
            }
        }
        if m.is_empty() {
            return Ok(Exit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    // Verifies: ESC-SRS-109
    #[test]
    fn pointer_creation_failure_exits_4_before_device_startup() {
        use std::cell::Cell;
        let started = Cell::new(false);
        let keyboard_created = Cell::new(false);
        let result = super::with_outputs(
            || {
                keyboard_created.set(true);
                Ok(())
            },
            |()| Err(std::io::ErrorKind::PermissionDenied.into()),
            |()| {
                started.set(true);
                Ok(crate::error::Exit::Success)
            },
        );
        let error = result.unwrap_err();
        assert_eq!(error.exit().code(), 4);
        assert!(keyboard_created.get());
        assert!(
            !started.get(),
            "device enumeration and grabs must not start"
        );
    }
    use super::*;

    // Verifies: ESC-SRS-063, ESC-SRS-064, ESC-SRS-100
    #[test]
    fn backend_selection() {
        use BackendArg::{Auto, Evdev, X11};
        assert_eq!(select(Auto, true, || true, true), Selected::Evdev);
        assert_eq!(
            select(Auto, true, || false, true),
            Selected::X11,
            "no readable keyboard"
        );
        assert_eq!(
            select(Auto, false, || true, true),
            Selected::X11,
            "no uinput"
        );
        assert_eq!(select(Auto, false, || true, false), Selected::None);
        assert_eq!(
            select(Evdev, true, || false, true),
            Selected::Evdev,
            "forced: keyboards may come later"
        );
        assert_eq!(
            select(Evdev, false, || true, true),
            Selected::None,
            "forced evdev never falls back"
        );
        assert_eq!(
            select(X11, true, || true, false),
            Selected::X11,
            "forced x11; fails later without DISPLAY"
        );
    }
    use crate::config::{Overrides, load};
    use escapepod_core::KeyCode;
    use escapepod_evdev::DeviceInfo;
    use escapepod_evdev::fake::FakeOpener;

    // Verifies: ESC-SRS-059
    #[test]
    fn socket_is_0660_with_the_control_group() {
        let dir = std::env::temp_dir().join(format!("escapepod-sock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        let gid = nix::unistd::getgid();
        let l = prepare_socket(&path, Some(gid)).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o660);
        assert_eq!(std::os::unix::fs::MetadataExt::gid(&meta), gid.as_raw());
        assert_eq!(
            prepare_socket(&path, Some(gid)).err().map(|e| e.kind()),
            Some(io::ErrorKind::AddrInUse)
        );
        drop(l);
        let private = prepare_socket(&path, None).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600,
            "stale socket replaced, owner-only without the group"
        );
        drop(private);
    }

    // Verifies: ESC-SRS-061, ESC-SRS-040
    #[test]
    fn monitor_redacts_unmanaged_keys_and_writes_lines() {
        let o = Overrides {
            map: Some(crate::keymap::parse("Caps_Lock=Escape").unwrap()),
            ..Overrides::default()
        };
        let cfg = load(None, &o, &[]).unwrap();
        let mut opener = FakeOpener::default();
        let h = opener.add_keyboard("/dev/input/event4");
        let info = DeviceInfo {
            path: "/dev/input/event4".into(),
            name: "kb".into(),
            vendor: 1,
            product: 1,
            kind: DeviceKind::Keyboard,
        };
        let mut m = Monitor::open(&mut opener, &cfg, &[info]);
        assert!(!h.borrow().grabbed, "monitor never grabs");
        let a = KeyCode::from_name("a").unwrap();
        h.borrow_mut().queue.extend([
            KeyEvent::press(a),
            KeyEvent::release(a),
            KeyEvent::press(KeyCode::CAPSLOCK),
            KeyEvent::release(KeyCode::CAPSLOCK),
        ]);
        let recs = m.ready(DeviceId(0), Instant::from_micros(10), false);
        assert_eq!(
            recs.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["other", "other", "KEY_CAPSLOCK", "KEY_CAPSLOCK"]
        );
        assert_eq!(recs[3].decision.as_deref(), Some("tap KEY_ESC"));
        let env = Env::tty(&[("SPACECRAFT_A11Y", "1")]);
        let (mode, _) = Mode::resolve(
            &crate::output::Flags::default(),
            &env,
            "escapepod event monitor".into(),
        )
        .unwrap();
        let text: String = recs.iter().map(|r| r.line(&mode)).collect();
        assert_eq!(text.lines().count(), 4);
        assert!(!text.contains("KEY_A") && !text.contains('\r') && !text.contains("\x1b["));
    }
}

// Rust guideline compliant 2026-05-18
