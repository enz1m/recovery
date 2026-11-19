// SPDX-License-Identifier: MIT
//! Restore from a backup (recovery environment only): find the encrypted
//! backup on an external drive, a network share or a backup server, choose a
//! snapshot and restore its personal files to an external drive. The
//! backup's own password unlocks it; enzimos-recovery-backup does the work.
use crate::i18n::label;
use enzui_widgets::{
    button::{ButtonKind, enz_button},
    combo_box::enz_combo_box,
    text::{TextTone, enz_text},
    text_field::enz_text_field_owned,
};
use iced::widget::{column, progress_bar, row};
use iced::{Element, Task};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use zeroize::Zeroizing;

const TOOL: &str = "/usr/lib/enzimos-recovery/enzimos-recovery-backup";

/// Where the backup is.
const KINDS: [&str; 3] = [
    "External drive",
    "Network share (SMB)",
    "Backup server (REST)",
];

#[derive(Clone)]
pub(crate) struct Secret(Zeroizing<String>);
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Kind(usize),
    Source(String),
    Folder(String),
    Location(String),
    User(String),
    SharePassword(Secret),
    Password(Secret),
    Target(String),
    Drives,
    List,
    Choose(usize),
    Restore,
    /// A progress fraction, or the tool's final answer.
    Progress(f64),
    Done(String, Result<Value, String>),
}

#[derive(Default)]
pub(crate) struct Page {
    kind: usize,
    drives: Vec<Value>,
    source: String,
    folder: String,
    location: String,
    user: String,
    share_password: Zeroizing<String>,
    password: Zeroizing<String>,
    target: String,
    snapshots: Vec<Value>,
    chosen: Option<usize>,
    busy: bool,
    progress: Option<f64>,
    error: String,
    done: String,
}

/// Whether this is the recovery environment (not the installed system).
pub(crate) fn available(demo: bool) -> bool {
    !demo && std::path::Path::new("/etc/enzimos-recovery-env").is_file()
}

fn drive_name(d: &Value) -> String {
    let label = d["label"].as_str().filter(|l| !l.is_empty());
    let size = d["size"]
        .as_u64()
        .map(|b| format!("{:.1} GB", b as f64 / 1e9))
        .unwrap_or_default();
    format!(
        "{} {}",
        label.unwrap_or_else(|| d["name"].as_str().unwrap_or("?")),
        size
    )
    .trim()
    .to_owned()
}

/// Run the tool once: the request on stdin, progress then a result line out.
fn call(action: &'static str, request: Value) -> Task<Message> {
    Task::run(
        iced::stream::channel(
            16,
            move |mut out: iced::futures::channel::mpsc::Sender<Message>| async move {
                use iced::futures::SinkExt;
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
                let worker = tokio::task::spawn_blocking(move || {
                    let result = (|| -> Result<Value, String> {
                        let mut child = std::process::Command::new("/usr/bin/sudo")
                            .args(["-n", TOOL])
                            .stdin(std::process::Stdio::piped())
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                            .map_err(|_| {
                                label("The backup tool is not available here.").to_owned()
                            })?;
                        let body = Zeroizing::new(request.to_string());
                        let mut input = child
                            .stdin
                            .take()
                            .ok_or_else(|| label("The backup tool did not answer.").to_owned())?;
                        input
                            .write_all(body.as_bytes())
                            .map_err(|_| label("The backup tool failed.").to_owned())?;
                        drop(input);
                        let stdout = child
                            .stdout
                            .take()
                            .ok_or_else(|| label("The backup tool did not answer.").to_owned())?;
                        let mut last = None;
                        for line in std::io::BufReader::new(stdout)
                            .lines()
                            .map_while(Result::ok)
                        {
                            let value: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                            if let Some(p) = value["progress"].as_f64() {
                                let _ = tx.send(Message::Progress(p));
                            } else {
                                last = Some(value);
                            }
                        }
                        let succeeded = child.wait().is_ok_and(|status| status.success());
                        match last {
                            Some(v) if succeeded && v.get("result").is_some() => {
                                Ok(v["result"].clone())
                            }
                            Some(v) => Err(v["error"]
                                .as_str()
                                .unwrap_or_else(|| label("The backup tool failed."))
                                .to_owned()),
                            None => Err(label("The backup tool did not answer.").to_owned()),
                        }
                    })();
                    let _ = tx.send(Message::Done(action.to_owned(), result));
                });
                while let Some(message) = rx.recv().await {
                    let _ = out.send(message).await;
                }
                let _ = worker.await;
            },
        ),
        |m| m,
    )
}

impl Page {
    fn source(&self) -> Value {
        match self.kind {
            0 => json!({"kind": "drive", "drive": self.source, "path": self.folder}),
            1 => {
                json!({"kind": "share", "location": self.location, "user": self.user, "share_password": self.share_password.as_str()})
            }
            _ => {
                json!({"kind": "rest", "location": self.location, "user": self.user, "server_password": self.share_password.as_str()})
            }
        }
    }

    pub(crate) fn busy(&self) -> bool {
        self.busy
    }

    pub(crate) fn enter(&mut self) -> Task<Message> {
        self.done.clear();
        self.update(Message::Drives)
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        if self.busy && !matches!(message, Message::Progress(_) | Message::Done(..)) {
            return Task::none();
        }
        if matches!(
            message,
            Message::Kind(_)
                | Message::Source(_)
                | Message::Folder(_)
                | Message::Location(_)
                | Message::User(_)
                | Message::SharePassword(_)
                | Message::Password(_)
        ) {
            self.snapshots.clear();
            self.chosen = None;
        }
        match message {
            Message::Kind(kind) => self.kind = kind.min(2),
            Message::Source(id) => self.source = id,
            Message::Folder(text) => self.folder = text,
            Message::Location(text) => self.location = text,
            Message::User(text) => self.user = text,
            Message::SharePassword(secret) => self.share_password = secret.0,
            Message::Password(secret) => self.password = secret.0,
            Message::Target(id) => self.target = id,
            Message::Drives => {
                self.busy = true;
                self.error.clear();
                return call("drives", json!({"action": "drives"}));
            }
            Message::List => {
                self.busy = true;
                self.error.clear();
                self.snapshots.clear();
                self.chosen = None;
                return call(
                    "snapshots",
                    json!({"action": "snapshots", "source": self.source(), "password": self.password.as_str()}),
                );
            }
            Message::Choose(index) if index < self.snapshots.len() => self.chosen = Some(index),
            Message::Restore => {
                let Some(snapshot) = self.chosen.and_then(|i| self.snapshots[i]["id"].as_str())
                else {
                    return Task::none();
                };
                self.busy = true;
                self.error.clear();
                self.progress = Some(0.0);
                return call(
                    "restore",
                    json!({"action": "restore", "source": self.source(), "password": self.password.as_str(), "snapshot": snapshot, "target": self.target}),
                );
            }
            Message::Progress(p) => self.progress = Some(p),
            Message::Done(action, result) => {
                self.busy = false;
                self.progress = None;
                match (action.as_str(), result) {
                    ("drives", Ok(rows)) => {
                        self.drives = rows
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|d| d["reason"] == "")
                            .collect();
                    }
                    ("snapshots", Ok(rows)) => {
                        self.snapshots = rows.as_array().cloned().unwrap_or_default();
                        if self.snapshots.is_empty() {
                            self.error = label("This backup has no snapshots.").to_owned();
                        }
                    }
                    ("restore", Ok(done)) => {
                        self.done = format!(
                            "{} \u{201c}{}\u{201d}",
                            label("Your files were restored to the folder"),
                            done["folder"].as_str().unwrap_or_default()
                        );
                    }
                    (_, Err(error)) => self.error = error,
                    _ => {}
                }
            }
            _ => {}
        }
        Task::none()
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        let kinds: Vec<String> = KINDS.iter().map(|k| label(k).to_owned()).collect();
        let names: Vec<String> = self.drives.iter().map(drive_name).collect();
        let pick = |id: &str| {
            self.drives
                .iter()
                .position(|d| d["id"] == id)
                .map(|i| names[i].clone())
        };
        let ids: Vec<String> = self
            .drives
            .iter()
            .map(|d| d["id"].as_str().unwrap_or_default().to_owned())
            .collect();
        let by_name = |ids: Vec<String>, names: Vec<String>| {
            move |name: String| {
                ids.get(names.iter().position(|n| *n == name).unwrap_or(usize::MAX))
                    .cloned()
                    .unwrap_or_default()
            }
        };
        let mut form = column![
            enz_combo_box(
                label("Where is the backup?"),
                kinds.clone(),
                Some(kinds[self.kind].clone()),
                move |k| { Message::Kind(KINDS.iter().position(|x| label(x) == k).unwrap_or(0)) }
            )
            .width(360)
            .into_element()
        ]
        .spacing(12);
        form = match self.kind {
            0 => form
                .push(
                    row![
                        enz_combo_box(label("Backup drive"), names.clone(), pick(&self.source), {
                            let f = by_name(ids.clone(), names.clone());
                            move |n| Message::Source(f(n))
                        })
                        .width(360)
                        .into_element(),
                        enz_button(label("Refresh"), Message::Drives)
                            .kind(ButtonKind::Secondary)
                            .into_element()
                    ]
                    .spacing(12)
                    .align_y(iced::Alignment::End),
                )
                .push(
                    enz_text_field_owned(
                        label("Folder on the drive"),
                        "enzimos-backup",
                        self.folder.clone(),
                        Message::Folder,
                    )
                    .into_element(),
                ),
            1 => form
                .push(
                    enz_text_field_owned(
                        label("Share"),
                        "smb://nas.local/backups/enzimos",
                        self.location.clone(),
                        Message::Location,
                    )
                    .into_element(),
                )
                .push(
                    row![
                        enz_text_field_owned(label("User"), "", self.user.clone(), Message::User)
                            .into_element(),
                        enz_text_field_owned(
                            label("Share password"),
                            "",
                            self.share_password.as_str().to_owned(),
                            |v| { Message::SharePassword(Secret(Zeroizing::new(v))) }
                        )
                        .secure(true)
                        .into_element()
                    ]
                    .spacing(12),
                ),
            _ => form
                .push(
                    enz_text_field_owned(
                        label("Server address"),
                        "https://backup.example.org/me",
                        self.location.clone(),
                        Message::Location,
                    )
                    .into_element(),
                )
                .push(
                    row![
                        enz_text_field_owned(label("User"), "", self.user.clone(), Message::User)
                            .into_element(),
                        enz_text_field_owned(
                            label("Server password"),
                            "",
                            self.share_password.as_str().to_owned(),
                            |v| { Message::SharePassword(Secret(Zeroizing::new(v))) }
                        )
                        .secure(true)
                        .into_element()
                    ]
                    .spacing(12),
                ),
        };
        form = form.push(
            row![
                enz_text_field_owned(
                    label("Backup password"),
                    "",
                    self.password.as_str().to_owned(),
                    |v| Message::Password(Secret(Zeroizing::new(v)))
                )
                .secure(true)
                .into_element(),
                enz_button(label("Find backups"), Message::List)
                    .kind(ButtonKind::Accent)
                    .enabled(!self.busy && !self.password.is_empty())
                    .into_element()
            ]
            .spacing(12)
            .align_y(iced::Alignment::End),
        );
        if !self.snapshots.is_empty() {
            let mut list = column![enz_text(label("Choose a snapshot")).into_element()].spacing(6);
            for (i, s) in self.snapshots.iter().enumerate().take(50) {
                let title = format!(
                    "{}  {}",
                    s["time"]
                        .as_str()
                        .unwrap_or_default()
                        .get(..16)
                        .unwrap_or_default()
                        .replace('T', " "),
                    s["host"].as_str().unwrap_or_default()
                );
                list = list.push(
                    enz_button(title, Message::Choose(i))
                        .kind(if self.chosen == Some(i) {
                            ButtonKind::Accent
                        } else {
                            ButtonKind::Secondary
                        })
                        .into_element(),
                );
            }
            form = form.push(iced::widget::scrollable(list).height(160));
            form = form.push(
                row![
                    enz_combo_box(
                        label("Restore personal files to"),
                        names.clone(),
                        pick(&self.target),
                        {
                            let f = by_name(ids, names);
                            move |n| Message::Target(f(n))
                        }
                    )
                    .width(360)
                    .into_element(),
                    enz_button(label("Restore"), Message::Restore)
                        .kind(ButtonKind::Accent)
                        .enabled(
                            !self.busy
                                && self.chosen.is_some()
                                && !self.target.is_empty()
                                && self.target != self.source
                        )
                        .into_element()
                ]
                .spacing(12)
                .align_y(iced::Alignment::End),
            );
        }
        if let Some(p) = self.progress {
            form = form.push(progress_bar(0.0..=1.0, p as f32).girth(6));
        }
        if !self.error.is_empty() {
            form = form.push(
                enz_text(self.error.clone())
                    .tone(TextTone::Danger)
                    .into_element(),
            );
        }
        if !self.done.is_empty() {
            form = form.push(enz_text(self.done.clone()).into_element());
        }
        form.into()
    }
}
