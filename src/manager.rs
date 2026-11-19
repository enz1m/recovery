use super::*;
use enzui_core::{
    drag::{DragOperation, ItemId, SourceCapabilities},
    menu_model::{MenuEntryModel, MenuModel},
    virtual_sequence::{ItemProvider, KeyedProvider, SequenceKey},
};
use enzui_widgets::{
    context_menu_host::{ContextMenuEvent, ContextMenuState},
    drag_drop_host::enz_drag_drop_host,
    drag_item_surface::{DragSurfaceItem, enz_drag_item_surface},
    modal_host::ModalHostEvent,
    toolbar_icon::enz_toolbar_icon,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Activate(bool),
    Key(iced::keyboard::Event),
    Select(bool, PathBuf, iced::keyboard::Modifiers),
    Open(bool, PathBuf),

    Navigate(bool, String),
    Scrolled(bool, f32),
    Context(bool, Option<PathBuf>, iced::Point),
    Menu(ContextMenuEvent),
    Action(bool, String),
    Name(String),
    Confirm,
    Dismiss,
    Modal(ModalHostEvent),
}
#[derive(Default)]
pub(crate) struct Rows(Vec<FileEntry>);
impl ItemProvider<FileEntry> for Rows {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn item(&self, i: u64) -> Option<FileEntry> {
        self.0.get(i as usize).cloned()
    }
}
impl KeyedProvider<FileEntry> for Rows {
    fn key(&self, i: u64) -> Option<SequenceKey> {
        self.0
            .get(i as usize)
            .map(|e| SequenceKey::from(e.path.to_string_lossy().into_owned()))
    }
}
pub(crate) struct Context {
    right: bool,
    model: MenuModel,
    state: ContextMenuState,
}
pub(crate) struct Dialog {
    right: bool,
    operation: String,
    paths: Vec<PathBuf>,
    name: String,
}
#[derive(Default)]
pub(crate) struct Manager {
    pub(crate) browse: Option<tokio::sync::mpsc::UnboundedSender<Value>>,
    pub(crate) context: Option<Context>,
    dialog: Option<Dialog>,
    clipboard: Option<(Value, Vec<PathBuf>)>,
    right_selected: BTreeSet<PathBuf>,
    active: bool,
    rows: [Rows; 2],
    scroll: [f32; 2],
    anchor: [Option<PathBuf>; 2],
    cache: HashMap<String, Value>,
    pub(crate) operation: String,
    pub(crate) operation_right: bool,
    pub(crate) operation_plan: Value,
}
impl Manager {
    pub(crate) fn remember(&mut self, right: bool, v: &Value, drive: &str) {
        let count = v["entries"].as_array().map_or(0, Vec::len);
        if count > 4096 {
            return;
        }
        let cached: usize = self
            .cache
            .values()
            .map(|entry| entry["entries"].as_array().map_or(0, Vec::len))
            .sum();
        if self.cache.len() >= 64 || cached + count > 16384 {
            self.cache.clear();
        }
        self.cache.insert(
            format!(
                "{}:{}",
                if right { drive } else { "system" },
                v["path"].as_str().unwrap_or("")
            ),
            v.clone(),
        );
    }
}
pub(crate) fn tool<'a>(icon: &str, label: &str, msg: Msg, enabled: bool) -> Element<'a, Msg> {
    enz_toolbar_icon(icon, msg)
        .aria_label(label)
        .tooltip(label)
        .enabled(enabled)
        .into_element()
}
fn item_count(n: u64) -> String {
    format!("{n} item{}", if n == 1 { "" } else { "s" })
}
fn event(e: Event) -> Msg {
    Msg::Manager(e)
}
pub(crate) fn entries(v: &Value) -> Vec<FileEntry> {
    v["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| {
            !e["name"]
                .as_str()
                .unwrap_or("")
                .starts_with(".enzimos-copy-report-")
        })
        .map(|e| {
            let p = e["path"].as_str().unwrap_or("");
            let n = e["name"].as_str().unwrap_or("");
            if e["directory"] == true {
                FileEntry::directory(p, n)
            } else {
                FileEntry::file(p, n, e["size"].as_u64().unwrap_or(0))
            }
        })
        .collect()
}
impl Recovery {
    pub(crate) fn refresh_views(&mut self) {
        self.fm.rows[0].0 = self.browser.visible().into_iter().cloned().collect();
        self.fm.rows[1].0 = self.destination.visible().into_iter().cloned().collect();
    }
    pub(crate) fn double_click(&mut self, right: bool, p: &PathBuf) -> bool {
        let now = std::time::Instant::now();
        let yes = self.last_click.as_ref().is_some_and(|(r, last, t)| {
            *r == right && last == p && now.duration_since(*t).as_millis() < 450
        });
        self.last_click = if yes {
            None
        } else {
            Some((right, p.clone(), now))
        };
        yes
    }
    fn endpoint(&self, right: bool) -> Value {
        if right {
            json!({"drive":self.drive})
        } else {
            json!({})
        }
    }
    fn current(&self, right: bool) -> String {
        if right {
            self.destination_path.clone()
        } else {
            self.path.clone()
        }
    }
    fn selection(&self, right: bool) -> &BTreeSet<PathBuf> {
        if right {
            &self.fm.right_selected
        } else {
            &self.selected
        }
    }
    fn selection_mut(&mut self, right: bool) -> &mut BTreeSet<PathBuf> {
        if right {
            &mut self.fm.right_selected
        } else {
            &mut self.selected
        }
    }
    fn browsing(&self) -> bool {
        matches!(
            self.action.as_str(),
            "files_list" | "files_destination" | "files_drives"
        )
    }
    pub(crate) fn destination_call(&mut self) -> Task<Msg> {
        self.call(json!({"action":"files_destination","drive":self.drive,"path":self.destination_path,"page":null}))
    }

    fn navigate(&mut self, right: bool, path: String) -> Task<Msg> {
        if self.busy {
            return Task::none();
        }
        self.fm.context = None;
        self.fm.active = right;
        self.error.clear();
        self.notice.clear();
        self.selection_mut(right).clear();
        self.fm.scroll[right as usize] = 0.0;
        let key = format!("{}:{}", if right { &self.drive } else { "system" }, path);
        if let Some(v) = self.fm.cache.get(&key).cloned() {
            let model = if right {
                &mut self.destination
            } else {
                &mut self.browser
            };
            model.set_entries(entries(&v));
            model.set_show_hidden(true);
        } else {
            if right {
                self.destination.set_entries(vec![]);
            } else {
                self.browser.set_entries(vec![]);
            }
        }
        self.refresh_views();
        let task = if right {
            self.destination_path = path;
            self.destination_call()
        } else {
            self.path = path;
            self.call(json!({"action":"files_list","path":self.path,"page":null}))
        };
        Task::batch([
            task,
            iced::widget::operation::scroll_to(
                if right {
                    "recovery-right-list"
                } else {
                    "recovery-left-list"
                },
                iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
                iced::widget::operation::Animation::Instant,
            ),
        ])
    }
    pub(crate) fn manager_update(&mut self, e: Event) -> Task<Msg> {
        match e {
            Event::Activate(r) => {
                self.fm.active = r;
                if !self.busy {
                    self.selection_mut(r).clear();
                }
            }
            Event::Key(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if self.busy
                    || self.fm.dialog.is_some()
                    || self.fm.context.as_ref().is_some_and(|c| c.state.is_open())
                {
                    return Task::none();
                }
                use iced::keyboard::{Key, key::Named};
                let r = self.fm.active;
                let action = match &key {
                    Key::Character(c) if modifiers.command() => match c.to_lowercase().as_str() {
                        "c" => Some("copy"),
                        "v" => Some("paste"),
                        "a" => Some("all"),
                        "n" if modifiers.shift() => Some("mkdir"),
                        _ => None,
                    },
                    Key::Named(Named::Delete) => Some("delete"),
                    Key::Named(Named::F2) => Some("rename"),
                    Key::Named(Named::F5) => Some("refresh"),
                    _ => None,
                };
                if let Some(a) = action {
                    return self.manager_update(Event::Action(r, a.into()));
                }
                if matches!(key, Key::Named(Named::ArrowUp)) && modifiers.alt() {
                    let parent = PathBuf::from(self.current(r))
                        .parent()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(if r { String::new() } else { "/".into() });
                    return self.navigate(r, parent);
                }
            }
            Event::Key(_) => {}
            Event::Scrolled(r, y) => self.fm.scroll[r as usize] = y,
            Event::Select(r, p, m) => {
                if !self.busy {
                    self.fm.active = r;
                    if m.command() || m.shift() {
                        self.last_click = None;
                    } else if self.double_click(r, &p)
                        && self.fm.rows[r as usize]
                            .0
                            .iter()
                            .any(|e| e.path == p && e.kind == EntryKind::Directory)
                    {
                        return self.manager_update(Event::Open(r, p));
                    }
                    if m.shift() {
                        let rows = &self.fm.rows[r as usize].0;
                        let anchor = self.fm.anchor[r as usize].as_ref().unwrap_or(&p);
                        let a = rows.iter().position(|e| &e.path == anchor).unwrap_or(0);
                        let b = rows.iter().position(|e| e.path == p).unwrap_or(a);
                        let range = rows[a.min(b)..=a.max(b)]
                            .iter()
                            .map(|e| e.path.clone())
                            .collect::<Vec<_>>();
                        let selected = self.selection_mut(r);
                        if !m.command() {
                            selected.clear();
                        }
                        selected.extend(range);
                    } else {
                        self.fm.anchor[r as usize] = Some(p.clone());
                        let selected = self.selection_mut(r);
                        if m.command() {
                            if !selected.remove(&p) {
                                selected.insert(p);
                            }
                        } else {
                            selected.clear();
                            selected.insert(p);
                        }
                    }
                }
            }
            Event::Open(r, p) => {
                if self.fm.rows[r as usize]
                    .0
                    .iter()
                    .any(|e| e.path == p && e.kind == EntryKind::Directory)
                {
                    return self.navigate(r, p.to_string_lossy().into_owned());
                }
            }
            Event::Navigate(r, p) => return self.navigate(r, p),
            Event::Context(r, p, at) => {
                if !self.busy {
                    self.fm.active = r;
                    self.last_click = None;
                    if let Some(p) = &p
                        && !self.selection(r).contains(p)
                    {
                        self.selection_mut(r).clear();
                        self.selection_mut(r).insert(p.clone());
                    }
                    let has = !self.selection(r).is_empty();
                    let folder = p.as_ref().is_some_and(|p| {
                        self.fm.rows[r as usize]
                            .0
                            .iter()
                            .any(|e| &e.path == p && e.kind == EntryKind::Directory)
                    });
                    let available = !r || !self.drive.is_empty();
                    let mut rows = vec![];
                    if folder {
                        rows.push(
                            MenuEntryModel::command("open", crate::i18n::label("Open folder"))
                                .icon("folder-open"),
                        );
                    }
                    if p.is_some() {
                        rows.push(
                            MenuEntryModel::command("copy", crate::i18n::label("Copy"))
                                .icon("copy")
                                .enabled(has),
                        );
                    }
                    rows.push(
                        MenuEntryModel::command("paste", crate::i18n::label("Paste here"))
                            .icon("clipboard-paste")
                            .enabled(available && self.fm.clipboard.is_some()),
                    );
                    rows.push(MenuEntryModel::separator("sep-edit"));
                    rows.push(
                        MenuEntryModel::command("mkdir", crate::i18n::label("New folder…"))
                            .icon("folder-plus")
                            .enabled(available),
                    );
                    if p.is_some() {
                        rows.push(
                            MenuEntryModel::command("rename", crate::i18n::label("Rename…"))
                                .icon("pencil")
                                .enabled(self.selection(r).len() == 1),
                        );
                        rows.push(
                            MenuEntryModel::command("delete", crate::i18n::label("Delete…"))
                                .icon("trash-2")
                                .enabled(has),
                        );
                    }
                    rows.push(MenuEntryModel::separator("sep-view"));
                    rows.push(
                        MenuEntryModel::command("all", crate::i18n::label("Select all"))
                            .icon("list-checks")
                            .enabled(!self.fm.rows[r as usize].0.is_empty()),
                    );
                    rows.push(
                        MenuEntryModel::command("refresh", crate::i18n::label("Refresh"))
                            .icon("refresh-cw")
                            .enabled(available),
                    );
                    let mut state = ContextMenuState::closed();
                    state.open(enzui_core::context_menu::InvocationKind::Pointer(
                        enzui_core::context_menu::Point::new(at.x, at.y),
                    ));
                    self.fm.context = Some(Context {
                        right: r,
                        model: MenuModel::new(rows),
                        state,
                    });
                }
            }
            Event::Menu(e) => {
                if let Some(c) = self.fm.context.as_mut()
                    && let Some(id) = enzui_widgets::context_menu_host::apply_event(
                        &mut c.model,
                        &mut c.state,
                        &e,
                    )
                {
                    let r = c.right;
                    self.fm.context = None;
                    return self.manager_update(Event::Action(r, id.0));
                }
            }
            Event::Action(r, action) => {
                if self.busy {
                    return Task::none();
                }
                self.fm.context = None;
                self.fm.active = r;
                match action.as_str() {
                    "open" => {
                        if let Some(p) = self.selection(r).first().cloned() {
                            return self.manager_update(Event::Open(r, p));
                        }
                    }
                    "copy" => {
                        self.fm.clipboard = Some((
                            self.endpoint(r),
                            self.selection(r).iter().cloned().collect(),
                        ));
                        self.notice = crate::i18n::format(
                            crate::i18n::localizer(),
                            "{} ready to paste",
                            &[("arg0", &(item_count(self.selection(r).len() as u64)))],
                        );
                    }
                    "paste" => {
                        if let Some((source, paths)) = self.fm.clipboard.clone()
                            && !paths.is_empty()
                        {
                            self.fm.operation = "copy".into();
                            self.fm.operation_right = r;
                            self.progress = Value::Null;
                            return self.call(json!({"action":"files_manager_plan","operation":"copy","source":source,"paths":paths,"destination":self.endpoint(r),"path":self.current(r)}));
                        }
                    }
                    "refresh" => {
                        self.fm.cache.clear();
                        return self.navigate(r, self.current(r));
                    }
                    "all" => {
                        let paths = self.fm.rows[r as usize]
                            .0
                            .iter()
                            .map(|e| e.path.clone())
                            .collect();
                        *self.selection_mut(r) = paths;
                    }
                    "delete" | "rename" | "mkdir" => {
                        let paths = self.selection(r).iter().cloned().collect::<Vec<_>>();
                        if action == "mkdir" || !paths.is_empty() {
                            let name = if action == "rename" {
                                paths[0]
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned()
                            } else {
                                String::new()
                            };
                            let focus = action != "delete";
                            self.fm.dialog = Some(Dialog {
                                right: r,
                                operation: action,
                                paths,
                                name,
                            });
                            if focus {
                                return iced::widget::operation::focus("recovery-file-name");
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Name(v) => {
                if let Some(d) = self.fm.dialog.as_mut() {
                    d.name = v;
                }
            }
            Event::Confirm => {
                if !self.busy
                    && let Some(d) = self.fm.dialog.take()
                {
                    self.fm.operation = d.operation.clone();
                    self.fm.operation_right = d.right;
                    return self.call(json!({"action":"files_manager_plan","operation":d.operation,"source":self.endpoint(d.right),"paths":d.paths,"path":self.current(d.right),"name":d.name}));
                }
            }
            Event::Dismiss => self.fm.dialog = None,
            Event::Modal(e) => match e {
                ModalHostEvent::Dismissed(_) => {
                    if !self.busy {
                        self.fm.dialog = None;
                    }
                }
                ModalHostEvent::FocusNext => return iced::widget::operation::focus_next(),
                ModalHostEvent::FocusPrevious => return iced::widget::operation::focus_previous(),
            },
        }
        Task::none()
    }
    pub(crate) fn manager_drop(&mut self, drop: enzui_core::drag::ItemDropRequest) -> Task<Msg> {
        let src = drop.request.source.0.as_str();
        let dst = drop.request.destination.0.as_str();
        if !matches!(src, "system" | "destination") || !matches!(dst, "system" | "destination") {
            return Task::none();
        }
        let r = dst == "destination";
        let from = src == "destination";
        let path = match drop.target {
            enzui_core::drag::DropTarget::OntoItem(id) => id.0.to_string(),
            _ => self.current(r),
        };
        self.fm.operation = "copy".into();
        self.fm.operation_right = r;
        self.progress = Value::Null;
        self.call(json!({"action":"files_manager_plan","operation":"copy","source":self.endpoint(from),"paths":drop.request.items.iter().map(|i|i.0.as_str()).collect::<Vec<_>>(),"destination":self.endpoint(r),"path":path}))
    }
    pub(crate) fn manager_done(&mut self, action: &str, v: Value) -> Task<Msg> {
        if action == "files_manager_plan" {
            self.fm.operation_plan = v.clone();
            return self.call(
                json!({"action":"files_manager_apply","plan":v["plan"],"digest":v["digest"]}),
            );
        }
        self.fm.cache.clear();
        self.notice = match self.fm.operation.as_str() {
            "copy" => crate::i18n::format(
                crate::i18n::localizer(),
                "Copied and verified {}",
                &[("arg0", &(item_count(v["items"].as_u64().unwrap_or(0))))],
            ),
            "delete" => crate::i18n::label("Items deleted").into(),
            "rename" => crate::i18n::label("Item renamed").into(),
            _ => crate::i18n::label("Folder created").into(),
        };
        if v["skipped"].as_u64().unwrap_or(0) > 0 {
            self.notice.push_str(&crate::i18n::format(
                crate::i18n::localizer(),
                " · {} unsupported items skipped",
                &[("arg0", &(v["skipped"].as_u64().unwrap_or(0)))],
            ));
        }
        let r = self.fm.operation_right;
        self.selection_mut(r).clear();
        if r {
            self.destination_call()
        } else {
            self.call(json!({"action":"files_list","path":self.path,"page":null}))
        }
    }
}

#[cfg(test)]
mod tests;

mod view;
