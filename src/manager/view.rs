use super::*;
use iced::widget::column;

impl Recovery {
    pub(crate) fn manager_overlays<'a>(&'a self, shell: Element<'a, Msg>) -> Element<'a, Msg> {
        let menu = self.fm.context.as_ref().map(|c| (&c.model, &c.state));
        let shell = enzui_widgets::context_menu_host::enz_optional_point_context_menu_host(
            shell,
            menu,
            |e| event(Event::Menu(e)),
        )
        .id_prefix("recovery-files-menu")
        .into_element();
        let dialog = self.fm.dialog.as_ref().map(|d| {
            let deleting = d.operation == "delete";
            let title = if deleting {
                crate::i18n::label("Delete selected items?")
            } else if d.operation == "rename" {
                crate::i18n::label("Rename")
            } else {
                crate::i18n::label("New folder")
            };
            let mut body = column![enz_text(title).size(TextSize::Title2)].spacing(18);
            if deleting {
                body = body.push(
                    enz_text(crate::i18n::label(
                        "These items will be permanently deleted. This cannot be undone.",
                    ))
                    .tone(TextTone::Muted),
                );
                let names = d
                    .paths
                    .iter()
                    .take(6)
                    .map(|p| p.file_name().unwrap_or_default().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("\n");
                body = body.push(enz_text(names));
                if d.paths.len() > 6 {
                    body = body.push(enz_text(format!("and {} more", d.paths.len() - 6)));
                }
            } else {
                body = body.push(
                    enzui_widgets::enz_text_field("", crate::i18n::text("Name"), &d.name, |v| {
                        event(Event::Name(v))
                    })
                    .on_submit(event(Event::Confirm))
                    .id("recovery-file-name".into())
                    .into_element(),
                );
            }
            body = body.push(
                row![
                    Space::new().width(Length::Fill),
                    enz_button(crate::i18n::label("Cancel"), event(Event::Dismiss)).into_element(),
                    enz_button(
                        if deleting {
                            crate::i18n::label("Delete")
                        } else if d.operation == "rename" {
                            crate::i18n::label("Rename")
                        } else {
                            crate::i18n::label("Create folder")
                        },
                        event(Event::Confirm)
                    )
                    .kind(if deleting {
                        ButtonKind::Destructive
                    } else {
                        ButtonKind::Accent
                    })
                    .enabled(deleting || !d.name.trim().is_empty())
                    .into_element()
                ]
                .spacing(10),
            );
            enzui_widgets::modal_surface::enz_modal_surface(body.into())
                .width(460.0)
                .padding(24.0)
                .into_element()
        });
        enzui_widgets::modal_host::enz_modal_host(shell, dialog, |e| event(Event::Modal(e)))
            .label(crate::i18n::label("File operation"))
            .into_element()
    }
    pub(super) fn file_row<'a>(&'a self, r: bool, e: &FileEntry) -> Element<'a, Msg> {
        let p = e.path.clone();
        let open = p.clone();
        let menu = p.clone();
        let click = p.clone();
        let select = p.clone();
        let isdir = e.kind == EntryKind::Directory;
        let icon = if isdir {
            "folder"
        } else {
            match e.path.extension().and_then(|s| s.to_str()).unwrap_or("") {
                "png" | "jpg" | "jpeg" => "file-image",
                "pdf" => "file-text",
                "zip" | "gz" => "file-archive",
                _ => "file",
            }
        };
        let label = enzui_widgets::icon_label::enz_icon_label(icon, e.name.clone())
            .size(TextSize::Body)
            .icon_color(if self.dark {
                EnzTheme::dark().palette.accent
            } else {
                EnzTheme::light().palette.accent
            })
            .icon_size(18.0)
            .slot_width(18.0)
            .spacing(10.0)
            .a11y(enzui_widgets::icon_label::IconLabelA11y::Decorative)
            .fill(true)
            .into_element();
        let content = row![
            label,
            enz_text(if isdir { String::new() } else { bytes(e.size) })
                .size(TextSize::Caption)
                .tone(TextTone::Muted)
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        enzui_widgets::list_row_surface::enz_list_row_surface(
            container(content).padding([0, 12]),
            event(Event::Select(r, p, iced::keyboard::Modifiers::default())),
        )
        .on_activate_with_modifiers(move |m| event(Event::Select(r, click.clone(), m)))
        .on_enter(move || event(Event::Open(r, open.clone())))
        .on_space(move || {
            event(Event::Select(
                r,
                select.clone(),
                iced::keyboard::Modifiers::CTRL,
            ))
        })
        .on_context(move |at| event(Event::Context(r, Some(menu.clone()), at)))
        .selected(self.selection(r).contains(&e.path))
        .selectable(true)
        .label(e.name.clone())
        .id_prefix(if r { "destination" } else { "system" })
        .row_height(34.0)
        .outer_gutter(6.0)
        .into_element()
    }
    pub(super) fn panel(&self, r: bool) -> Element<'_, Msg> {
        let path = self.current(r);
        let parent = PathBuf::from(&path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let available = !r || !self.drive.is_empty();
        let up = tool(
            "arrow-up",
            crate::i18n::label("Parent folder"),
            event(Event::Navigate(r, parent)),
            available
                && !self.busy
                && if r {
                    !path.is_empty() && path != "."
                } else {
                    path != "/"
                },
        );
        let mut crumbs = row![].spacing(2).align_y(Alignment::Center);
        let root = if r {
            crate::i18n::label("External drive")
        } else {
            crate::i18n::label("System")
        };
        crumbs = crumbs.push(
            enz_button(
                root,
                event(Event::Navigate(
                    r,
                    if r { String::new() } else { "/".into() },
                )),
            )
            .kind(ButtonKind::Quiet)
            .enabled(!self.busy && available)
            .into_element(),
        );
        let parts: Vec<_> = PathBuf::from(&path)
            .components()
            .filter_map(|c| {
                if let std::path::Component::Normal(s) = c {
                    Some(s.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();
        let mut acc = if r {
            PathBuf::new()
        } else {
            PathBuf::from("/")
        };
        for (i, part) in parts.iter().enumerate() {
            acc.push(part);
            if i == 0 && parts.len() > 2 {
                crumbs = crumbs
                    .push(enz_icon("chevron-right").size(12.0))
                    .push(enz_text("…").tone(TextTone::Muted));
            }
            if i + 2 >= parts.len() {
                crumbs = crumbs.push(enz_icon("chevron-right").size(12.0)).push(
                    enz_button(
                        part.clone(),
                        event(Event::Navigate(r, acc.to_string_lossy().into_owned())),
                    )
                    .kind(ButtonKind::Quiet)
                    .enabled(!self.busy)
                    .into_element(),
                );
            }
        }
        let toolbar = row![
            up,
            crumbs,
            Space::new().width(Length::Fill),
            tool(
                "folder-plus",
                crate::i18n::label("New folder"),
                event(Event::Action(r, "mkdir".into())),
                available && !self.busy
            ),
            tool(
                "refresh-cw",
                crate::i18n::label("Refresh"),
                event(Event::Action(r, "refresh".into())),
                available && !self.busy
            )
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        let rows = &self.fm.rows[r as usize];
        let offset = self.fm.scroll[r as usize];
        let list: Element<'_, Msg> = if !available {
            container(
                column![
                    enz_icon("usb").size(36.0).tint_theme_accent(),
                    enz_text(crate::i18n::label("Connect an external drive")).tone(TextTone::Muted)
                ]
                .spacing(12)
                .align_x(Alignment::Center),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else if rows.0.is_empty() {
            container(
                enz_text(if self.busy && self.browsing() {
                    ""
                } else {
                    crate::i18n::label("This folder is empty")
                })
                .tone(TextTone::Muted),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        } else {
            enzui_widgets::virtual_list::enz_virtual_list(rows, 34.0, move |_, e| {
                self.file_row(r, e)
            })
            .id(if r {
                "recovery-right-list".into()
            } else {
                "recovery-left-list".into()
            })
            .scroll_offset(offset)
            .viewport_height(500.0)
            .on_scroll(move |v| event(Event::Scrolled(r, v.absolute_offset().y)))
            .height(Length::Fill)
            .into_element()
        };
        let items = rows
            .0
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let y = *i as f32 * 34.0 - offset;
                y + 34.0 >= 0.0 && y < 650.0
            })
            .map(|(i, e)| {
                (
                    i,
                    e.path.to_string_lossy().into_owned(),
                    e.name.clone(),
                    e.kind == EntryKind::Directory,
                )
            })
            .collect::<Vec<_>>();
        let surface = enz_drag_item_surface(
            &self.drag,
            list,
            if r { "destination" } else { "system" },
            vec![],
        )
        .items_from_size(move |size| {
            items
                .iter()
                .map(|(i, id, name, dir)| {
                    DragSurfaceItem::new(
                        ItemId(id.clone()),
                        iced::Rectangle {
                            x: 6.0,
                            y: *i as f32 * 34.0 - offset,
                            width: (size.width - 12.0).max(0.0),
                            height: 34.0,
                        },
                        name.clone(),
                    )
                    .drag_source(available)
                    .drop_target(available && *dir)
                })
                .collect()
        })
        .selected(
            self.selection(r)
                .iter()
                .map(|p| ItemId(p.to_string_lossy().into_owned()))
                .collect(),
        )
        .source_capabilities(SourceCapabilities {
            copy: available,
            move_: false,
            delete: false,
        })
        .destination_operations(if available {
            vec![DragOperation::Copy]
        } else {
            vec![]
        })
        .accepts_background(available)
        .allows_captured_press(true);
        let list =
            enzui_widgets::context_target::enz_context_target(Element::from(surface), move |at| {
                event(Event::Context(r, None, at))
            })
            .id(if r {
                "right-background"
            } else {
                "left-background"
            })
            .label(crate::i18n::label("Folder actions"));
        let list =
            iced::widget::mouse_area(Element::from(list)).on_press(event(Event::Activate(r)));
        let header = row![
            enz_text(crate::i18n::label("Name"))
                .size(TextSize::Caption)
                .tone(TextTone::Muted),
            Space::new().width(Length::Fill),
            enz_text(crate::i18n::label("Size"))
                .size(TextSize::Caption)
                .tone(TextTone::Muted)
        ]
        .align_y(Alignment::Center);
        column![
            container(toolbar).padding([8, 6]),
            container(header).padding(iced::Padding {
                top: 8.0,
                bottom: 8.0,
                left: 46.0,
                right: 18.0
            }),
            list
        ]
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
    }
    pub(crate) fn file_manager(&self) -> Element<'_, Msg> {
        let drives: Vec<_> = self.drives.iter().filter(|d| d["reason"] == "").collect();
        let options = drives
            .iter()
            .map(|d| {
                format!(
                    "{} · {}",
                    d["label"]
                        .as_str()
                        .unwrap_or(crate::i18n::label("External drive")),
                    bytes(d["size"].as_u64().unwrap_or(0))
                )
            })
            .collect::<Vec<_>>();
        let selected = drives
            .iter()
            .position(|d| d["id"].as_str() == Some(&self.drive));
        let ids = drives
            .iter()
            .map(|d| d["id"].as_str().unwrap_or("").to_string())
            .collect::<Vec<_>>();
        let choose = enzui_widgets::combo_box::enz_combo_box("", options, None, |_| Msg::Clear)
            .selected_index(selected)
            .on_select_index(move |i| Msg::ChooseDrive(ids[i].clone()))
            .placeholder(crate::i18n::text("Select an external drive"))
            .width(Length::Fill)
            .enabled(!self.busy)
            .into_element();
        let left = row![
            enz_icon("computer").size(20.0).tint_theme_accent(),
            enz_text(crate::i18n::label("System files")).size(TextSize::Title3),
            Space::new().width(Length::Fill)
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let right = row![
            enz_icon("hard-drive").size(20.0).tint_theme_accent(),
            choose,
            tool(
                "unplug",
                crate::i18n::label("Eject drive"),
                Msg::Eject,
                !self.busy && !self.drive.is_empty()
            ),
            tool(
                "refresh-cw",
                crate::i18n::label("Find drives"),
                Msg::RefreshDrives,
                !self.busy
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let headers = row![
            container(left).width(Length::Fill),
            container(right).width(Length::Fill)
        ]
        .spacing(24)
        .align_y(Alignment::Center);
        let panels = enz_drag_drop_host(
            &self.drag,
            row![
                self.panel(false),
                container(Space::new())
                    .width(1)
                    .height(Length::Fill)
                    .style(|t| container::Style {
                        background: Some(
                            enzui_widgets::theme_lookup::palette(t)
                                .control_border
                                .into()
                        ),
                        ..Default::default()
                    }),
                self.panel(true)
            ]
            .spacing(6)
            .height(Length::Fill),
        )
        .on_item_drop(Msg::Drop)
        .into_element();
        let r = self.fm.active;
        let mut status = row![
            enz_text(if self.notice.is_empty() {
                if self.selection(r).is_empty() {
                    String::new()
                } else {
                    crate::i18n::format(
                        crate::i18n::localizer(),
                        "{} selected",
                        &[("arg0", &(self.selection(r).len()))],
                    )
                }
            } else {
                self.notice.clone()
            })
            .size(TextSize::Caption)
            .tone(TextTone::Muted),
            Space::new().width(Length::Fill),
            tool(
                "copy",
                crate::i18n::label("Copy selected items"),
                event(Event::Action(r, "copy".into())),
                !self.busy && !self.selection(r).is_empty()
            ),
            tool(
                "clipboard-paste",
                crate::i18n::label("Paste"),
                event(Event::Action(r, "paste".into())),
                !self.busy && self.fm.clipboard.is_some() && (!r || !self.drive.is_empty())
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if self.busy
            && matches!(
                self.action.as_str(),
                "files_manager_plan" | "files_manager_apply"
            )
            && self.fm.operation == "copy"
        {
            let done = self.progress["bytes"].as_u64().unwrap_or(0) as f32;
            let total = self.progress["total"].as_u64().unwrap_or(1).max(1) as f32;
            status = status
                .push(
                    container(progress_bar(0.0..=1.0, (done / total).min(0.99)).girth(4))
                        .width(140),
                )
                .push(tool(
                    "x",
                    crate::i18n::label("Cancel copy"),
                    Msg::Cancel,
                    self.action == "files_manager_apply" && !self.cancelling,
                ));
        }
        let body = column![
            container(headers).padding([8, 12]),
            panels,
            container(status).padding([6, 12])
        ]
        .spacing(4)
        .height(Length::Fill);
        container(body).padding([8, 12]).height(Length::Fill).into()
    }
    pub(crate) fn snapshot_details(&self) -> Element<'_, Msg> {
        let mut col = column![
            enz_text(
                self.details["description"]
                    .as_str()
                    .unwrap_or(crate::i18n::label("System restore point"))
            )
            .tone(TextTone::Muted)
        ]
        .spacing(10);
        if let Some(summary) = self.details["summary"].as_str() {
            col = col.push(enz_text(crate::i18n::text(summary)));
        }
        col = col.push(
            enz_text(crate::i18n::label(
                "Package comparison only · personal files are kept",
            ))
            .tone(TextTone::Muted),
        );
        if let Some(changes) = self.details["changes"].as_array().filter(|c| !c.is_empty()) {
            let limit = if self.show_changes { changes.len() } else { 6 };
            let mut list = column![].spacing(6);
            for c in changes.iter().take(limit) {
                list = list.push(enz_text(c.as_str().unwrap_or("")).size(TextSize::Caption));
            }
            col = col.push(
                enz_scroll_view(list)
                    .height(if self.show_changes { 210 } else { 130 })
                    .into_element(),
            );
            if changes.len() > 6 {
                col = col.push(
                    enz_button(
                        if self.show_changes {
                            crate::i18n::label("Show less")
                        } else {
                            crate::i18n::label("Show all package changes")
                        },
                        Msg::Changes,
                    )
                    .into_element(),
                );
            }
        }
        col.into()
    }
}
