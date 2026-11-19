//! Native recovery navigation, guarded helper requests and a non-destructive demo.

mod backend;
mod backup_page;
mod i18n;
mod log_transfer;
mod manager;
mod native_backend;
mod snapshots;
use enzui_core::file_browser::{EntryKind, FileBrowserModel, FileEntry};
use enzui_theme::EnzTheme;
use enzui_widgets::app_shell::{
    DesktopApplicationIdentity, ShellLayout, ShellTokens, ShellWindowEvent, ShellWindowState,
    enz_app_shell,
};
use enzui_widgets::button::ButtonKind;
use enzui_widgets::title_bar::enz_title_bar;
use enzui_widgets::title_bar_gesture::WindowCommand;
use enzui_widgets::{
    BannerTone, TextSize, TextTone, enz_banner, enz_button, enz_divider, enz_icon, enz_scroll_view,
    enz_secure_field, enz_text,
};
use iced::widget::{Space, container, progress_bar, row, text};
use iced::{Alignment, Element, Length, Task};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::PathBuf};
use zeroize::{Zeroize, Zeroizing};
#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Page {
    #[default]
    Home,
    Auth,
    Terminal,
    Snapshots,

    Files,
    Logs,
    Power,
    Finished,
    /// Restore from an encrypted backup (recovery environment only).
    Backup,
}
#[derive(Clone)]
struct PrivatePassword(Zeroizing<String>);
impl std::fmt::Debug for PrivatePassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}
fn password_message(value: String) -> Msg {
    Msg::Password(PrivatePassword(Zeroizing::new(value)))
}
#[derive(Debug, Clone)]
enum Msg {
    Manager(manager::Event),
    Password(PrivatePassword),
    User(String),
    Reveal,
    Authenticate,
    Go(Page),
    Back,
    ChooseSnapshot(usize),
    Restore,
    Logs,
    Theme(bool),
    Reboot,
    Shutdown,
    Finish(String),
    Window(WindowCommand),
    WindowState(ShellWindowEvent),
    SharedTheme(Box<enzui_core::theme_file::integration::ThemeEvent>),
    Backend(backend::Event),

    Clear,
    ChooseDrive(String),
    RefreshDrives,

    Copy,
    Cancel,
    Eject,
    Terminal,

    Drop(enzui_core::drag::ItemDropRequest),
    Tick,
    Changes,
    ShowQr,
    CloseQr,
    LogsTick,
    LogBackend(backend::Event),
    QrModal(enzui_widgets::modal_host::ModalHostEvent),
    Backup(backup_page::Message),
    DismissError,
    DismissNotice,
}
#[derive(Default)]
struct Recovery {
    chrome: ShellWindowState,
    backup: backup_page::Page,
    themes: Option<enzui_core::theme_file::integration::ThemeIntegration>,
    fm: manager::Manager,
    log_transfer: Option<log_transfer::Transfer>,
    logs_pending: bool,
    log_cancel: Option<tokio::sync::oneshot::Sender<()>>,
    pending_confirmation: bool,
    dark: bool,
    revealed: bool,
    demo: bool,
    password: Zeroizing<String>,
    user: String,
    accounts: Vec<Value>,
    authenticated: bool,
    page: Page,
    power_return: Page,
    after_auth: Page,
    terminal_after_auth: bool,
    state: Value,
    snapshot: Option<usize>,
    busy: bool,
    error: String,
    notice: String,
    logs: String,
    browser: FileBrowserModel,
    path: String,
    file_page: usize,
    more: bool,
    selected: BTreeSet<PathBuf>,
    drives: Vec<Value>,
    drive: String,
    plan: Value,
    copy_result: Value,
    progress: Value,
    destination: FileBrowserModel,
    destination_path: String,
    destination_page: usize,
    destination_more: bool,
    right_offset: usize,
    drag: enzui_widgets::drag_drop_host::DragHostChannel,
    last_click: Option<(bool, PathBuf, std::time::Instant)>,

    details: Value,
    show_changes: bool,
    refresh_destination: bool,
    loading: f32,
    started: Option<std::time::Instant>,
    durations: std::collections::HashMap<String, f32>,
    action: String,
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    cancelling: bool,
}
impl Recovery {
    fn new(demo: bool) -> (Self, Task<Msg>) {
        let (themes, theme_boot) = enzui_core::theme_file::integration::ThemeIntegration::boot();
        let mut app = Self {
            dark: themes.theme().mode() == enzui_theme::EnzThemeMode::Dark,
            themes: Some(themes),
            demo,
            pending_confirmation: !demo
                && std::fs::read_to_string("/run/enzimos-recovery-needed")
                    .unwrap_or_default()
                    .starts_with("Recovery confirmation required"),
            path: "/home".into(),
            ..Default::default()
        };
        if !demo {
            let state = std::fs::read_to_string("/run/enzimos-recovery-needed").unwrap_or_default();
            for prefix in [
                "Snapshot recovery did not finish",
                "Recovery metadata needs attention",
            ] {
                if let Some(detail) = state.trim().strip_prefix(prefix) {
                    app.notice = format!("{}{detail}", crate::i18n::text(prefix));
                    break;
                }
            }
        }
        let task = app.call(json!({"action":"accounts"}));
        (
            app,
            Task::batch([
                task,
                theme_boot.map(|event| Msg::SharedTheme(Box::new(event))),
            ]),
        )
    }
    fn call(&mut self, request: Value) -> Task<Msg> {
        self.busy = true;
        self.loading = 0.03;
        self.started = Some(std::time::Instant::now());
        self.error.clear();
        self.refresh_views();
        if request["action"] == "terminal" {
            self.page = Page::Terminal;
        }
        self.action = request["action"].as_str().unwrap_or("").into();
        self.cancelling = false;
        if matches!(
            self.action.as_str(),
            "files_list" | "files_destination" | "files_drives"
        ) {
            if let Some(tx) = &self.fm.browse
                && tx.send(request.clone()).is_ok()
            {
                return Task::none();
            }
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let _ = tx.send(request);
            self.fm.browse = Some(tx);
            return Task::run(
                backend::browse(self.user.clone(), self.password.clone(), self.demo, rx),
                Msg::Backend,
            );
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.cancel = Some(tx);
        Task::run(
            backend::run(
                request,
                self.user.clone(),
                self.password.clone(),
                self.demo,
                rx,
            ),
            Msg::Backend,
        )
    }
    fn enter(&mut self, page: Page) -> Task<Msg> {
        if page == Page::Backup && !backup_page::available(self.demo) {
            return Task::none();
        }
        self.error.clear();
        self.notice.clear();
        self.fm.context = None;
        self.log_transfer = None;
        if let Some(tx) = self.log_cancel.take() {
            let _ = tx.send(());
        }
        self.logs_pending = false;
        if !self.authenticated && matches!(page, Page::Files | Page::Snapshots | Page::Logs) {
            self.after_auth = page;
            self.page = Page::Auth;
            return iced::widget::operation::focus("recovery-admin-password");
        }
        self.page = page;
        if page == Page::Files {
            self.refresh_destination = true;
        }
        match page {
            Page::Files => self.call(json!({"action":"files_drives"})),
            Page::Snapshots => self.call(json!({"action":"inspect"})),
            Page::Logs => self.call(json!({"action":"logs"})),
            Page::Backup => self.backup.enter().map(Msg::Backup),
            _ => Task::none(),
        }
    }
    fn update(&mut self, msg: Msg) -> Task<Msg> {
        crate::i18n::localizer()
            .clone()
            .scope(|| self.localized_update(msg))
    }

    fn localized_update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Manager(event) => return self.manager_update(event),
            Msg::Window(WindowCommand::Close) => {
                if !self.busy && !self.backup.busy() {
                    if self.page != Page::Power {
                        self.power_return = self.page;
                    }
                    self.error.clear();
                    self.notice.clear();
                    self.page = Page::Power;
                }
            }
            Msg::Window(command) => {
                return self.chrome.run_command(command);
            }
            Msg::WindowState(event) => return self.chrome.update(event).map(Msg::WindowState),
            Msg::SharedTheme(event) => {
                if let Some(themes) = &mut self.themes {
                    let task = themes.update(*event);
                    self.dark = themes.theme().mode() == enzui_theme::EnzThemeMode::Dark;
                    return task.map(|event| Msg::SharedTheme(Box::new(event)));
                }
            }
            Msg::Tick => {
                if self.busy {
                    let estimate =
                        self.durations.get(&self.action).copied().unwrap_or(
                            match self.action.as_str() {
                                "restore" => 12.0,
                                "inspect" | "snapshot_details" => 2.0,
                                _ => 1.0,
                            },
                        );
                    let elapsed = self
                        .started
                        .map(|t| t.elapsed().as_secs_f32())
                        .unwrap_or(0.0);
                    self.loading = (1.0 - (-elapsed / estimate.max(0.1)).exp()).min(0.95);
                }
            }
            Msg::Changes => self.show_changes = !self.show_changes,
            Msg::DismissError => self.error.clear(),
            Msg::DismissNotice => self.notice.clear(),

            Msg::Drop(drop) => {
                if !self.busy && drop.request.operation == enzui_core::drag::DragOperation::Copy {
                    return self.manager_drop(drop);
                }
            }
            Msg::Theme(v) => {
                self.dark = v;
                if let Some(themes) = &mut self.themes {
                    themes.set_mode_override(Some(if v {
                        enzui_theme::EnzThemeMode::Dark
                    } else {
                        enzui_theme::EnzThemeMode::Light
                    }));
                }
            }
            Msg::Password(v) => self.password = v.0,
            Msg::User(v) => {
                if self.user != v {
                    self.password.zeroize();
                    self.authenticated = false;
                    self.fm.browse = None;
                    self.revealed = false;
                }
                self.user = v;
            }
            Msg::Reveal => self.revealed = !self.revealed,
            Msg::Authenticate => {
                if !self.busy && !self.password.is_empty() && !self.user.is_empty() {
                    return self.call(json!({"action":"authenticate"}));
                }
            }
            Msg::Backup(message) => {
                if self.page == Page::Backup && backup_page::available(self.demo) {
                    return self.backup.update(message).map(Msg::Backup);
                }
            }
            Msg::Go(page) => {
                if !self.busy && !self.backup.busy() {
                    return self.enter(page);
                }
            }
            Msg::Back => {
                if !self.busy && !self.backup.busy() {
                    let page = match self.page {
                        Page::Power => self.power_return,
                        _ => Page::Home,
                    };
                    self.terminal_after_auth = false;
                    return self.enter(page);
                }
            }
            Msg::ChooseSnapshot(i) => {
                if !self.busy && self.state["choices"][i]["available"] != false {
                    self.snapshot = Some(i);
                    self.page = Page::Snapshots;
                    self.details = Value::Null;
                    self.show_changes = false;
                    let c = &self.state["choices"][i];
                    return self.call(json!({"action":"snapshot_details","birth":c["birth"],"generation":c["generation"],"digest":self.state["digest"]}));
                }
            }
            Msg::Restore => {
                if !self.busy
                    && let Some(i) = self.snapshot
                {
                    let c = &self.state["choices"][i];
                    return self.call(json!({"action":"restore","birth":c["birth"],"generation":c["generation"],"digest":self.state["digest"]}));
                }
            }
            Msg::Finish(id) => {
                if !self.busy {
                    return self
                        .call(json!({"action":"finish","id":id,"digest":self.state["digest"]}));
                }
            }
            Msg::ShowQr => {
                self.log_transfer = Some(log_transfer::Transfer::new(&self.logs, &[&self.password]))
            }
            Msg::CloseQr => self.log_transfer = None,
            Msg::QrModal(e) => match e {
                enzui_widgets::modal_host::ModalHostEvent::Dismissed(_) => self.log_transfer = None,
                enzui_widgets::modal_host::ModalHostEvent::FocusNext => {
                    return iced::widget::operation::focus_next();
                }
                enzui_widgets::modal_host::ModalHostEvent::FocusPrevious => {
                    return iced::widget::operation::focus_previous();
                }
            },
            Msg::LogsTick => {
                if self.page == Page::Logs && !self.logs_pending && !self.busy {
                    self.logs_pending = true;
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    self.log_cancel = Some(tx);
                    return Task::run(
                        backend::run(
                            json!({"action":"logs"}),
                            self.user.clone(),
                            self.password.clone(),
                            self.demo,
                            rx,
                        ),
                        Msg::LogBackend,
                    );
                }
            }
            Msg::LogBackend(backend::Event::Done(_, result)) => {
                self.logs_pending = false;
                self.log_cancel = None;
                if self.page == Page::Logs {
                    match result {
                        Ok(v) => {
                            self.logs = v["logs"].as_str().unwrap_or("").into();
                        }
                        Err(e) => self.error = e,
                    }
                    if let Some(q) = self.log_transfer.as_mut() {
                        q.refresh(&self.logs, &[&self.password]);
                    }
                }
            }
            Msg::LogBackend(_) => {}
            Msg::Logs => return self.enter(Page::Logs),

            Msg::Clear => self.selected.clear(),
            Msg::ChooseDrive(id) => {
                self.drive = id;
                self.destination_path.clear();
                self.destination_page = 0;
                self.right_offset = 0;
                return self.destination_call();
            }
            Msg::RefreshDrives => {
                if !self.busy {
                    self.refresh_destination = true;
                    return self.call(json!({"action":"files_drives"}));
                }
            }

            Msg::Copy => {
                if !self.busy && self.plan["plan"].is_string() {
                    self.progress = json!({"bytes":0,"total":self.plan["bytes"],"files":0,"total_files":self.plan["files"]});
                    return self.call(json!({"action":"files_copy","plan":self.plan["plan"],"digest":self.plan["digest"]}));
                }
            }
            Msg::Cancel => {
                if (self.action == "files_copy"
                    || (self.action == "files_manager_apply" && self.fm.operation == "copy"))
                    && let Some(tx) = self.cancel.take()
                {
                    self.cancelling = true;
                    let _ = tx.send(());
                }
            }
            Msg::Eject => {
                if !self.busy {
                    return self.call(json!({"action":"files_eject","drive":self.drive}));
                }
            }
            Msg::Terminal => {
                if !self.busy {
                    if !self.authenticated {
                        self.after_auth = Page::Home;
                        self.terminal_after_auth = true;
                        self.page = Page::Auth;
                        // Focus the password, as the other three tools do.
                        return iced::widget::operation::focus("recovery-admin-password");
                    } else {
                        return self.call(json!({"action":"terminal"}));
                    }
                }
            }
            Msg::Backend(backend::Event::Progress(v)) => self.progress = v,
            Msg::Backend(backend::Event::Done(action, result)) => {
                if action == "browse_closed" {
                    self.fm.browse = None;
                }
                if let Some(start) = self.started.take() {
                    self.durations
                        .insert(action.clone(), start.elapsed().as_secs_f32());
                }
                self.busy = false;
                self.cancel = None;
                match result {
                    Err(e) => {
                        self.error = e;
                        if action == "authenticate" {
                            return iced::widget::operation::focus("recovery-admin-password");
                        }
                        self.refresh_views();
                        if action == "files_copy" {
                            self.plan = Value::Null;
                            self.page = Page::Files;
                        }
                    }
                    Ok(v) => {
                        match action.as_str() {
                            "files_manager_plan" | "files_manager_apply" => {
                                return self.manager_done(&action, v);
                            }
                            "accounts" => {
                                self.accounts =
                                    v["accounts"].as_array().cloned().unwrap_or_default();
                                if let Some(a) = self.accounts.first() {
                                    self.user = a["name"].as_str().unwrap_or("").into();
                                }
                            }
                            "inspect" | "authenticate" => {
                                if action == "inspect" {
                                    self.state = v;
                                }
                                self.authenticated = true;
                                if self.page == Page::Auth {
                                    if self.terminal_after_auth {
                                        self.terminal_after_auth = false;
                                        self.page = Page::Home;
                                        return self.call(json!({"action":"terminal"}));
                                    }
                                    return self.enter(self.after_auth);
                                }
                            }
                            "logs" => self.logs = v["logs"].as_str().unwrap_or("").into(),
                            "files_list" => {
                                self.path = v["path"].as_str().unwrap_or("/home").into();
                                self.file_page = v["page"].as_u64().unwrap_or(0) as usize;
                                self.more = v["more"].as_bool().unwrap_or(false);
                                let entries = v["entries"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|e| {
                                        let p = e["path"].as_str().unwrap_or("");
                                        let n = e["name"].as_str().unwrap_or("");
                                        if e["directory"] == true {
                                            FileEntry::directory(p, n)
                                        } else {
                                            FileEntry::file(p, n, e["size"].as_u64().unwrap_or(0))
                                        }
                                    })
                                    .collect();
                                self.browser.set_entries(entries);
                                self.browser.set_show_hidden(true);
                                self.fm.remember(false, &v, &self.drive);
                                self.refresh_views();
                                if self.refresh_destination {
                                    self.refresh_destination = false;
                                    if !self.drive.is_empty() {
                                        return self.destination_call();
                                    }
                                }
                            }
                            "snapshot_details" => self.details = v,
                            "files_destination" => {
                                self.destination_path = v["path"].as_str().unwrap_or("").into();
                                self.destination_page = v["page"].as_u64().unwrap_or(0) as usize;
                                self.destination_more = v["more"].as_bool().unwrap_or(false);
                                self.destination.set_entries(manager::entries(&v));
                                self.destination.set_show_hidden(true);
                                self.fm.remember(true, &v, &self.drive);
                                self.refresh_views();
                            }
                            "files_drives" => {
                                self.drives = v["drives"].as_array().cloned().unwrap_or_default();
                                if !self.drives.iter().any(|d| {
                                    d["id"].as_str() == Some(&self.drive) && d["reason"] == ""
                                }) {
                                    self.drive.clear();
                                    self.destination.set_entries(vec![]);
                                    self.refresh_views();
                                }
                                return self.call(
                                    json!({"action":"files_list","path":self.path,"page":null}),
                                );
                            }
                            "files_plan" => {
                                self.plan = v;
                                return self.update(Msg::Copy);
                            }
                            "files_copy" => {
                                let count = v["items"]
                                    .as_u64()
                                    .unwrap_or(v["files"].as_u64().unwrap_or(0));
                                let skipped = v["skipped"].as_u64().unwrap_or(0);
                                let source = if skipped == 0 {
                                    "Copied and verified {count} items"
                                } else {
                                    "Copied and verified {count} items; {skipped} skipped"
                                };
                                self.notice = crate::i18n::format(
                                    crate::i18n::localizer(),
                                    source,
                                    &[("count", &count), ("skipped", &skipped)],
                                );
                                self.copy_result = v;
                                self.page = Page::Files;
                                self.plan = Value::Null;
                                self.selected.clear();
                                return self.destination_call();
                            }
                            "files_eject" => {
                                self.notice =
                                    crate::i18n::label("The drive can now be safely disconnected.")
                                        .into();
                                self.drive.clear();
                                self.destination.set_entries(vec![]);
                                self.refresh_views();
                            }
                            "terminal" => {
                                self.page = Page::Home;
                                self.notice=crate::i18n::label("Terminal closed. Choose another recovery tool when you are ready.").into();
                                self.refresh_views();
                                return Task::none();
                            }
                            _ => {
                                if action == "finish" {
                                    self.pending_confirmation = false;
                                }
                                self.notice = v["message"]
                                    .as_str()
                                    .unwrap_or(crate::i18n::label("Operation completed."))
                                    .into();
                                self.page = Page::Finished;
                                if action == "restore" && !self.demo {
                                    return self.update(Msg::Reboot);
                                }
                            }
                        }
                    }
                }
            }
            Msg::Reboot | Msg::Shutdown => {
                if !self.busy && !self.backup.busy() && !self.demo {
                    let action = if matches!(msg, Msg::Reboot) {
                        "reboot"
                    } else {
                        "poweroff"
                    };
                    // systemctl only queues the job and returns at once; a refusal
                    // must not leave "restarting automatically" on screen forever.
                    let status = std::process::Command::new("/usr/bin/sudo")
                        .args(["-n", "/usr/bin/systemctl", action])
                        .status();
                    if !status.is_ok_and(|s| s.success()) {
                        self.notice = if action == "reboot" {
                            crate::i18n::label("The computer could not be restarted automatically. Use the Restart button or the power button.").into()
                        } else {
                            crate::i18n::label("The computer could not be shut down automatically. Use the power button.").into()
                        };
                    }
                }
            }
        }
        Task::none()
    }
}
fn bytes(n: u64) -> String {
    let (value, unit) = if n >= 1024 * 1024 * 1024 {
        (n as f64 / (1024.0 * 1024.0 * 1024.0), "GiB")
    } else if n >= 1024 * 1024 {
        (n as f64 / (1024.0 * 1024.0), "MiB")
    } else if n >= 1024 {
        (n as f64 / 1024.0, "KiB")
    } else {
        return format!("{} B", i18n::decimal(&n.to_string()));
    };
    format!("{} {unit}", i18n::decimal(&format!("{value:.1}")))
}

fn snapshot_label(v: &Value) -> String {
    v["ordinal"]
        .as_u64()
        .map(|n| {
            crate::i18n::format(
                crate::i18n::localizer(),
                "System snapshot {n}",
                &[("n", &(n))],
            )
        })
        .unwrap_or_else(|| crate::i18n::label("System snapshot").into())
}
fn window_settings() -> iced::window::Settings {
    let identity = DesktopApplicationIdentity::new("org.enzimos.Recovery").unwrap();
    let mut settings = identity.window_settings(ShellLayout::TitlebarContent);
    settings.size = iced::Size::new(1080.0, 740.0);
    settings
}
fn main() -> iced::Result {
    i18n::localizer().scope(localized_main)
}

fn localized_main() -> iced::Result {
    let demo = std::env::args().any(|a| a == "--demo");
    let settings = window_settings();
    enzui_widgets::app_setup::run_with_services(
        iced::application(
            move || Recovery::new(demo),
            Recovery::update,
            Recovery::view,
        )
        .title(crate::i18n::label("System Recovery"))
        .theme(|s: &Recovery| {
            s.themes.as_ref().map_or_else(
                || {
                    if s.dark {
                        EnzTheme::dark().iced_theme()
                    } else {
                        EnzTheme::light().iced_theme()
                    }
                },
                |themes| themes.iced_theme(),
            )
        })
        .subscription(|s: &Recovery| {
            iced::Subscription::batch([
                s.chrome.subscription().map(Msg::WindowState),
                s.themes
                    .as_ref()
                    .map_or_else(iced::Subscription::none, |themes| {
                        themes
                            .subscription()
                            .map(|event| Msg::SharedTheme(Box::new(event)))
                    }),
                iced::window::close_requests().map(|_| Msg::Window(WindowCommand::Close)),
                if s.page == Page::Logs {
                    iced::time::every(std::time::Duration::from_secs(1)).map(|_| Msg::LogsTick)
                } else {
                    iced::Subscription::none()
                },
                if s.busy {
                    iced::time::every(std::time::Duration::from_millis(100)).map(|_| Msg::Tick)
                } else {
                    iced::Subscription::none()
                },
                if s.page == Page::Files {
                    iced::keyboard::listen().map(|e| Msg::Manager(manager::Event::Key(e)))
                } else {
                    iced::Subscription::none()
                },
            ])
        })
        .window(settings)
        .exit_on_close_request(enzui_core::close_guard::EXIT_ON_CLOSE_REQUEST),
        enzui_widgets::app_setup::AppServices::default().localizer(i18n::localizer().clone()),
    )
}
#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;

#[path = "main/view.rs"]
mod view;
