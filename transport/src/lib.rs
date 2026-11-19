// SPDX-License-Identifier: MIT
//! Ordinary-user pipe transport for the separately authenticated native helper.
use enzimos_platform::{Error, Result, files, json, require};
use serde_json::Value;
use std::{
    fs::File,
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};
use zeroize::Zeroizing;
/// Installed, root-owned executable used for authenticated recovery.
pub const HELPER: &str = "/usr/lib/enzimos-recovery/enzimos-recovery-auth";
/// Maximum serialized request bytes, excluding the terminating newline.
pub const MAX_REQUEST: usize = 65536;
/// Maximum retained bytes in one helper response.
pub const MAX_RESPONSE: usize = 32 * 1024 * 1024;
struct PrivateWriter(Zeroizing<Vec<u8>>);
impl Write for PrivateWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_REQUEST {
            return Err(std::io::Error::other("recovery request byte bound"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Serialize an accounts query or authenticated request without exposing secrets in arguments.
///
/// # Errors
/// Rejects invalid credentials, unknown accounts-query fields and oversized requests.
pub fn frame(request: &Value, user: &str, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    require(request.is_object(), "recovery request object required")?;
    let mut output = PrivateWriter(Zeroizing::new(Vec::with_capacity(MAX_REQUEST + 1)));
    if request["action"] == "accounts" {
        json::fields(request, "action")?;
        serde_json::to_writer(&mut output, request)?;
    } else {
        require(
            !user.is_empty()
                && user.len() <= 64
                && !user.starts_with('-')
                && user
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
            "administrator name refused",
        )?;
        require(
            !password.is_empty()
                && password.len() <= 4096
                && !password.contains(['\n', '\r', '\0']),
            "administrator password refused",
        )?;
        output.write_all(b"{\"user\":")?;
        serde_json::to_writer(&mut output, user)?;
        output.write_all(b",\"password\":")?;
        serde_json::to_writer(&mut output, password)?;
        output.write_all(b",\"request\":")?;
        serde_json::to_writer(&mut output, request)?;
        output.write_all(b"}")?;
    }
    output.0.push(b'\n');
    Ok(output.0)
}
#[allow(unsafe_code, reason = "Linux syscall ABI (syscall).")]
fn signal(fd: &File, signal: i32) -> Result<()> {
    // SAFETY: the borrowed File owns a live pidfd; the null siginfo pointer requests no extra signal data.
    let result = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd.as_raw_fd(),
            signal,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    require(
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH),
        "owned recovery helper signal refused",
    )
}
async fn retire(mut child: Child, fd: File) -> Result<()> {
    signal(&fd, libc::SIGTERM)?;
    if let Ok(result) = tokio::time::timeout(Duration::from_secs(15), child.wait()).await {
        result?;
        return Ok(());
    }
    signal(&fd, libc::SIGKILL)?;
    tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| Error("owned recovery helper termination uncertain".into()))??;
    Ok(())
}
/// Owns one helper process, its identity handle and private request/reply pipes.
pub struct Client {
    child: Option<Child>,
    pidfd: Option<File>,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
/// A validated reply from the helper protocol.
pub enum Message {
    /// The helper is ready to receive its authenticated request.
    Ready,
    /// The helper accepted the administrator credentials.
    Authenticated,
    /// Nonterminal progress from an authenticated operation.
    Progress(Value),
    /// The single terminal result.
    Result(Value),
    /// A bounded terminal failure description.
    Failure(String),
}
/// Enforces reply ordering and one terminal result per request.
pub struct Replies {
    accounts: bool,
    ready: bool,
    authenticated: bool,
    finished: bool,
}
impl Replies {
    /// Begin an accounts-only or authenticated reply sequence.
    #[must_use]
    pub fn new(accounts: bool) -> Self {
        Self {
            accounts,
            ready: false,
            authenticated: false,
            finished: false,
        }
    }
    /// Validate and advance the reply sequence.
    ///
    /// # Errors
    /// Rejects malformed, out-of-order and repeated terminal envelopes.
    pub fn accept(&mut self, value: Value) -> Result<Message> {
        require(
            !self.finished && value.is_object(),
            "unexpected or repeated recovery result",
        )?;
        if value.get("error").is_some() {
            json::fields(&value, "error")?;
            let error = value["error"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 2000)
                .ok_or_else(|| Error("invalid recovery failure envelope".into()))?;
            self.finished = true;
            return Ok(Message::Failure(error.chars().take(500).collect()));
        }
        if value.get("ready").is_some() {
            json::fields(&value, "ready")?;
            require(
                !self.accounts && !self.ready && !self.authenticated && value["ready"] == true,
                "recovery handoff out of order",
            )?;
            self.ready = true;
            return Ok(Message::Ready);
        }
        if value.get("authenticated").is_some() {
            json::fields(&value, "authenticated")?;
            require(
                !self.accounts
                    && self.ready
                    && !self.authenticated
                    && value["authenticated"] == true,
                "recovery authentication out of order",
            )?;
            self.authenticated = true;
            return Ok(Message::Authenticated);
        }
        if value.get("progress").is_some() {
            json::fields(&value, "progress")?;
            require(
                self.authenticated && value["progress"].is_object(),
                "recovery progress before authentication",
            )?;
            return Ok(Message::Progress(value["progress"].clone()));
        }
        require(
            self.accounts || self.authenticated,
            "recovery result before authentication",
        )?;
        self.finished = true;
        Ok(Message::Result(value))
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        if let (Some(child), Some(fd)) = (self.child.take(), self.pidfd.take()) {
            let _ = signal(&fd, libc::SIGTERM);
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = retire(child, fd).await;
                });
            }
        }
    }
}
impl Client {
    /// Launch the retained inode of the trusted installed helper.
    ///
    /// # Errors
    /// Rejects an untrusted installation or a failed process/pipe setup.
    pub fn spawn() -> Result<Self> {
        let path = Path::new(HELPER);
        let parent = files::directory(path.parent().unwrap(), Some(0))?;
        let file = files::open_relative(&parent, Path::new(path.file_name().unwrap()))?;
        let metadata = file.metadata()?;
        require(
            metadata.uid() == 0
                && metadata.mode() & 0o7022 == 0
                && metadata.mode() & 0o111 != 0
                && metadata.nlink() == 1
                && files::read_file(&file, 32 * 1024 * 1024, Some(0), true)?
                    .starts_with(b"\x7fELF"),
            "native recovery helper installation untrusted",
        )?;
        // Execute the retained ELF inode; a concurrent replacement cannot
        // redirect this launch after the trust and header checks.
        Self::spawn_command(Command::new(format!("/proc/self/fd/{}", file.as_raw_fd())))
    }
    #[allow(unsafe_code, reason = "Linux syscall ABI (syscall).")]
    fn spawn_command(mut command: Command) -> Result<Self> {
        command
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .kill_on_drop(false)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn()?;
        let pid = child
            .id()
            .ok_or_else(|| Error("native recovery helper did not start".into()))?;
        // SAFETY: pid names the owned child and zero flags select the Linux pidfd_open ABI.
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
        if raw < 0 {
            let _ = child.start_kill();
            return Err(Error("owned recovery helper pidfd unavailable".into()));
        }
        // SAFETY: the successful syscall returned a fresh descriptor; this File takes sole ownership.
        let pidfd = unsafe { File::from_raw_fd(raw) };
        let input = child
            .stdin
            .take()
            .ok_or_else(|| Error("native recovery input unavailable".into()))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| Error("native recovery output unavailable".into()))?;
        Ok(Self {
            child: Some(child),
            pidfd: Some(pidfd),
            input: Some(input),
            output: BufReader::new(output),
        })
    }
    /// Send the initial query with bounded, zeroizing credential serialization.
    ///
    /// # Errors
    /// Reports invalid framing or a failed/timed-out pipe write.
    pub async fn send_initial(
        &mut self,
        request: &Value,
        user: &str,
        password: Zeroizing<String>,
        browse: bool,
    ) -> Result<()> {
        let bytes = frame(request, user, &password)?;
        tokio::time::timeout(
            Duration::from_secs(5),
            self.input
                .as_mut()
                .ok_or_else(|| Error("recovery input closed".into()))?
                .write_all(&bytes),
        )
        .await
        .map_err(|_| Error("recovery request send deadline".into()))??;
        if !browse {
            self.input.take();
        }
        Ok(())
    }
    /// Send a subsequent navigation request to an authenticated browsing connection.
    ///
    /// # Errors
    /// Reports invalid requests or a failed/timed-out pipe write.
    pub async fn send_browse(&mut self, request: &Value) -> Result<()> {
        require(
            matches!(
                request["action"].as_str(),
                Some("files_list" | "files_destination" | "files_drives")
            ),
            "browsing connection is read-only",
        )?;
        let mut bytes = json::canonical(request)?;
        require(bytes.len() <= MAX_REQUEST, "browse request byte bound")?;
        bytes.push(b'\n');
        tokio::time::timeout(
            Duration::from_secs(5),
            self.input
                .as_mut()
                .ok_or_else(|| Error("browsing input closed".into()))?
                .write_all(&bytes),
        )
        .await
        .map_err(|_| Error("browse request send deadline".into()))??;
        Ok(())
    }
    /// Read one bounded JSON envelope before the supplied deadline.
    ///
    /// # Errors
    /// Reports a timeout, oversized or malformed response, or a pipe read failure.
    pub async fn next(&mut self, timeout: Duration) -> Result<Option<Value>> {
        let read = async {
            let mut bytes = Vec::new();
            loop {
                let available = self.output.fill_buf().await?;
                if available.is_empty() {
                    require(bytes.is_empty(), "truncated recovery response")?;
                    return Ok(None);
                }
                let take = available
                    .iter()
                    .position(|b| *b == b'\n')
                    .map_or(available.len(), |n| n + 1);
                require(
                    bytes.len().saturating_add(take) <= MAX_RESPONSE + 1,
                    "recovery response line bound",
                )?;
                bytes.extend_from_slice(&available[..take]);
                self.output.consume(take);
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                    break;
                }
            }
            Ok(Some(json::decode(&bytes, MAX_RESPONSE)?))
        };
        tokio::time::timeout(timeout, read)
            .await
            .map_err(|_| Error("recovery response deadline; no automatic retry".into()))?
    }
    /// Retire the owned helper using its retained process identity.
    ///
    /// # Errors
    /// Reports an unconfirmed signal or termination deadline.
    pub async fn cancel(&mut self) -> Result<()> {
        self.input.take();
        let child = self
            .child
            .take()
            .ok_or_else(|| Error("recovery helper already retired".into()))?;
        let fd = self
            .pidfd
            .take()
            .ok_or_else(|| Error("recovery helper identity missing".into()))?;
        retire(child, fd).await
    }
    /// Close input and wait for normal helper completion.
    ///
    /// # Errors
    /// Reports failure to confirm termination or an unsuccessful helper exit.
    pub async fn finish(&mut self) -> Result<()> {
        self.input.take();
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| Error("recovery helper already retired".into()))?;
        let status = tokio::time::timeout(Duration::from_secs(20), child.wait())
            .await
            .map_err(|_| Error("recovery helper exit deadline".into()))??;
        self.child.take();
        self.pidfd.take();
        require(
            status.success(),
            "authentication failed or recovery operation refused",
        )
    }
}

#[cfg(test)]
mod tests;
