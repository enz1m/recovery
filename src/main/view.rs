use super::*;
use iced::widget::column;

/// The action footer below the page divider.
const FOOTER_HEIGHT: f32 = 77.0;

/// Recovery's own app icon (the enzimOS icon theme's org.enzimos.Recovery), in full colour.
const RECOVERY_ICON: &[u8] = include_bytes!("../../assets/org.enzimos.Recovery.svg");

fn recovery_icon<'a>(size: f32) -> Element<'a, Msg> {
    iced::widget::svg(iced::widget::svg::Handle::from_memory(RECOVERY_ICON))
        .width(size)
        .height(size)
        .into()
}

impl Recovery {
    pub(super) fn heading(&self) -> (&str, &str, &str) {
        match self.page {
            Page::Home => (
                "life-buoy",
                crate::i18n::label("System Recovery"),
                crate::i18n::label("Choose what you need to do"),
            ),
            Page::Auth => (
                "lock-keyhole",
                crate::i18n::label("Unlock recovery tools"),
                crate::i18n::label("Use an administrator account to continue"),
            ),
            Page::Terminal => (
                "terminal",
                crate::i18n::label("Recovery terminal"),
                crate::i18n::label("A full-screen console for command-line recovery work"),
            ),
            Page::Files => (
                "folder-open",
                crate::i18n::label("Back up your files"),
                crate::i18n::label("Drag files from your system to the external drive"),
            ),
            Page::Snapshots => (
                "history",
                crate::i18n::label("Restore a system snapshot"),
                crate::i18n::label("Return system files to an earlier working state"),
            ),
            Page::Logs => (
                "file-text",
                crate::i18n::label("Boot diagnostics"),
                crate::i18n::label("Output from the current boot"),
            ),
            Page::Backup => (
                "hard-drive-download",
                crate::i18n::label("Restore from a backup"),
                crate::i18n::label("Bring back your files from an encrypted backup"),
            ),
            Page::Power => (
                "power",
                crate::i18n::label("Power options"),
                crate::i18n::label("Restart or shut down this computer"),
            ),
            Page::Finished => {
                if self.action == "restore" {
                    (
                        "rotate-cw",
                        crate::i18n::label("Restoring system"),
                        crate::i18n::label("The computer will restart automatically"),
                    )
                } else {
                    ("circle-check", crate::i18n::label("Recovery completed"), "")
                }
            }
        }
    }
    pub(super) fn view(&self) -> Element<'_, Msg> {
        crate::i18n::localizer().view(|| self.localized_view())
    }

    fn localized_view(&self) -> Element<'_, Msg> {
        let title = || {
            let toggle =
                enzui_widgets::switch::switch(self.dark, Msg::Theme).colors(|theme, state| {
                    let mut c = enzui_widgets::switch::default_colors(theme, state);
                    let p = enzui_widgets::theme_lookup::palette(theme);
                    c.thumb = p.surface_elevated;
                    if !state.on {
                        c.track = p.control_border;
                    }
                    c
                });
            let leading: Element<'_, Msg> = if self.page == Page::Home {
                row![
                    enz_icon("sun").size(16.0),
                    toggle,
                    enz_icon("moon").size(16.0)
                ]
                .spacing(8.0)
                .align_y(Alignment::Center)
                .into()
            } else {
                row![
                    enzui_widgets::toolbar_icon::enz_toolbar_icon("chevron-left", Msg::Back)
                        .aria_label(crate::i18n::label("Back"))
                        .enabled(!self.busy)
                        .into_element(),
                    enz_text(if self.page == Page::Files {
                        crate::i18n::label("Files")
                    } else {
                        crate::i18n::label("System Recovery")
                    })
                    .size(TextSize::Body)
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .into()
            };
            let bar = enz_title_bar().push(leading);
            if self.demo {
                bar.push(
                    enzui_widgets::enz_badge(crate::i18n::label("Preview — no system changes"))
                        .tone(enzui_widgets::BadgeTone::Warning)
                        .into_element(),
                )
                .into_element()
            } else {
                bar.into_element()
            }
        };
        let shell = enz_app_shell(|| self.content())
            .layout(ShellLayout::TitlebarContent)
            .tokens(ShellTokens {
                show_close: !self.busy,
                ..ShellTokens::default()
            })
            .on_window_gesture(Msg::Window)
            .maximized(self.chrome.maximized())
            .fullscreen(self.chrome.fullscreen())
            .titlebar(title)
            .into_element();
        let shell = self.manager_overlays(shell);
        let shell = enzui_widgets::modal_host::enz_modal_host(
            shell,
            self.log_transfer.as_ref().map(|q| self.qr_dialog(q)),
            Msg::QrModal,
        )
        .label(crate::i18n::label("Boot log QR code"))
        .policy(
            enzui_widgets::modal_host::modal_dismiss_policy()
                .outside(enzui_core::dismissal::OutsidePolicy::Ignore),
        )
        .into_element();
        enzui_widgets::toast_host::enz_toast_layer(
            shell,
            self.notices(),
            // Above the footer and its button on the pages that have one.
            enzui_widgets::toast_host::ToastOverlayPlacement::default().bottom_inset(
                if matches!(self.page, Page::Auth | Page::Finished) {
                    FOOTER_HEIGHT
                } else {
                    0.0
                },
            ),
        )
    }
    /// Operation errors and results, as dismissible notices in the
    /// bottom-right corner. They stay until dismissed or the next operation
    /// or page clears them. The snapshot page keeps its own restore banner,
    /// and the file manager reports results in its status line.
    fn notices(&self) -> Option<Element<'_, Msg>> {
        use enzui_widgets::notice_stack::{EnzNotice, enz_notice_stack};
        use enzui_widgets::toast::ToastTone;
        let mut notices = Vec::new();
        if !self.error.is_empty() && self.page != Page::Snapshots {
            notices.push(
                EnzNotice::new(
                    "recovery-error",
                    crate::i18n::label("Could not complete the operation"),
                    ToastTone::Error,
                )
                .description(crate::i18n::text(&self.error))
                .on_dismiss(Msg::DismissError),
            );
        }
        if !self.notice.is_empty() && self.page != Page::Files {
            notices.push(
                EnzNotice::new(
                    "recovery-notice",
                    crate::i18n::text(&self.notice),
                    ToastTone::Info,
                )
                .on_dismiss(Msg::DismissNotice),
            );
        }
        (!notices.is_empty()).then(|| enz_notice_stack(notices).into_element())
    }
    pub(super) fn tile<'a>(
        &self,
        icon: &'a str,
        title: &'a str,
        description: &'a str,
        message: Msg,
    ) -> Element<'a, Msg> {
        enzui_widgets::focus_button::focus_button(
            column![
                row![
                    enz_icon(icon).size(32.0).tint_theme_accent(),
                    Space::new().width(Length::Fill),
                    enz_icon("chevron-right")
                        .size(18.0)
                        .tint_theme_tone(enzui_widgets::icon::IconTone::Muted)
                ]
                .align_y(Alignment::Center),
                enz_text(title).size(TextSize::Title3),
                enz_text(description).tone(TextTone::Muted)
            ]
            .spacing(16.0),
            message,
        )
        .id(iced::advanced::widget::Id::from(format!(
            "recovery-tool-{title}"
        )))
        .padding(24)
        .width(Length::Fill)
        .height(190)
        .style_focus(|theme, status| {
            let p = enzui_widgets::theme_lookup::palette(theme);
            let hover = matches!(
                status,
                enzui_widgets::focus_button::FocusStatus::Hovered
                    | enzui_widgets::focus_button::FocusStatus::Pressed
            );
            // A card surface with the decorative hairline (`border`), not the
            // control outline (`control_border`, the 3:1 rim checkboxes use):
            // the tiles are content, and a dark rule around each one made the
            // page look like a form from another decade. Hover lifts the fill;
            // focus alone takes the accent ring.
            let focused = status == enzui_widgets::focus_button::FocusStatus::Focused;
            iced::widget::button::Style {
                background: Some(if hover { p.surface_hover } else { p.surface }.into()),
                text_color: p.text,
                border: iced::Border {
                    color: if focused { p.focus_ring } else { p.border },
                    width: if focused { 2.0 } else { 1.0 },
                    radius: 14.0.into(),
                },
                shadow: iced::Shadow {
                    color: if hover {
                        p.shadow
                    } else {
                        iced::Color::TRANSPARENT
                    },
                    offset: iced::Vector::new(0.0, 2.0),
                    blur_radius: 10.0,
                },
                ..Default::default()
            }
        })
        .into()
    }
    pub(super) fn content(&self) -> Element<'_, Msg> {
        if self.page == Page::Files {
            return self.file_manager();
        }
        if self.page == Page::Snapshots {
            return self.snapshots_view();
        }
        if self.page == Page::Logs {
            return self.logs_view();
        }
        let (icon, title, subtitle) = self.heading();
        // The start page introduces the app, so it carries Recovery's own icon.
        let glyph: Element<'_, Msg> = if self.page == Page::Home {
            recovery_icon(48.0)
        } else {
            enz_icon(icon).size(40.0).tint_theme_accent().into()
        };
        let mut body = column![
            row![
                glyph,
                column![
                    enz_text(title).size(TextSize::Title1),
                    enz_text(subtitle).tone(TextTone::Muted)
                ]
                .spacing(8.0)
            ]
            .spacing(20.0)
            .align_y(Alignment::Center)
        ]
        .spacing(22.0);
        if self.page == Page::Files {
            body = column![
                row![
                    enz_icon("folder-open").size(26.0).tint_theme_accent(),
                    enz_text(crate::i18n::label("Back up files")).size(TextSize::Title2)
                ]
                .spacing(12)
                .align_y(Alignment::Center)
            ]
            .spacing(14);
        }
        if self.demo {
            body = body.push(
                enz_banner(crate::i18n::label("Preview — no system changes"))
                    .tone(BannerTone::Info)
                    .into_element(),
            );
        }
        if self.busy && self.action == "terminal" {
            body = body
                .push(Space::new().height(36))
                .push(
                    enz_text(crate::i18n::label(
                        "Type exit in the terminal to return here",
                    ))
                    .size(TextSize::Title2),
                )
                .push(
                    enz_text(crate::i18n::label(
                        "If you switched away from the console, press Ctrl+Alt+F8 to return to it.",
                    ))
                    .tone(TextTone::Muted),
                );
        } else if self.busy && self.action == "files_copy" {
            let done = self.progress["bytes"].as_u64().unwrap_or(0);
            let total = self.progress["total"].as_u64().unwrap_or(1).max(1);
            body = body.push(
                container(
                    container(
                        progress_bar(0.0..=1.0, (done as f64 / total as f64).min(0.99) as f32)
                            .girth(6),
                    )
                    .width(440),
                )
                .center_x(Length::Fill)
                .center_y(300),
            );
        } else if self.busy && self.action != "accounts" {
            body = body.push(
                container(container(progress_bar(0.0..=1.0, self.loading).girth(6)).width(280))
                    .center_x(Length::Fill)
                    .center_y(260),
            );
        } else {
            match self.page {
                Page::Home => {
                    // Two tiles a row on a wide window, one column below
                    // 640 px, where two columns elided every description.
                    body = body.push(
                        enzui_widgets::enz_responsive(move |size: iced::Size| {
                            let tiles: Vec<Element<'_, Msg>> = vec![
                                self.tile(
                                    "folder-open",
                                    crate::i18n::label("Back up files"),
                                    crate::i18n::label("Copy important files to a USB drive or external hard drive."),
                                    Msg::Go(Page::Files)
                                ),
                                if crate::backup_page::available(self.demo) {
                                    self.tile(
                                        "hard-drive-download",
                                        crate::i18n::label("Restore from a backup"),
                                        crate::i18n::label("Bring back your files from an encrypted backup on a drive, a network share or a server."),
                                        Msg::Go(Page::Backup)
                                    )
                                } else {
                                self.tile(
                                    "history",
                                    if self.pending_confirmation {
                                        crate::i18n::label("Finish recovery")
                                    } else {
                                        crate::i18n::label("Restore a snapshot")
                                    },
                                    if self.pending_confirmation {
                                        crate::i18n::label("Confirm the restored system so normal updates can resume.")
                                    } else {
                                        crate::i18n::label("Restore system files while keeping your personal files.")
                                    },
                                    Msg::Go(Page::Snapshots)
                                )
                                },

                                self.tile(
                                    "terminal",
                                    crate::i18n::label("Open terminal"),
                                    crate::i18n::label("Use a full-screen command line for recovery work."),
                                    Msg::Terminal
                                ),
                                self.tile(
                                    "file-text",
                                    crate::i18n::label("Boot diagnostics"),
                                    crate::i18n::label("Read warnings to help find what went wrong."),
                                    Msg::Logs
                                )
                            ];
                            if size.width < 640.0 {
                                let el: Element<'_, Msg> = column(tiles).spacing(16).into();
                                el
                            } else {
                                let mut rows = column![].spacing(20);
                                let mut it = tiles.into_iter();
                                while let Some(left) = it.next() {
                                    let mut r = row![left].spacing(20);
                                    if let Some(right) = it.next() {
                                        r = r.push(right);
                                    }
                                    rows = rows.push(r);
                                }
                                rows.into()
                            }
                        })
                        .height(Length::Shrink),
                    );
                }
                Page::Auth => {
                    let mut users = row![].spacing(12);
                    for a in &self.accounts {
                        let name = a["name"].as_str().unwrap_or("");
                        users = users.push(
                            enz_button(
                                format!("{} ({name})", a["label"].as_str().unwrap_or(name)),
                                Msg::User(name.into()),
                            )
                            .kind(if self.user == name {
                                ButtonKind::Accent
                            } else {
                                ButtonKind::Secondary
                            })
                            .into_element(),
                        );
                    }
                    body=body.push(Space::new().height(26)).push(users).push(enz_secure_field(&self.password,password_message,self.revealed,Msg::Reveal).id("recovery-admin-password".into()).placeholder(crate::i18n::text("Administrator password")).on_submit(Msg::Authenticate).enabled(!self.busy).into_element()).push(enz_text(crate::i18n::label("The disk passphrase unlocks storage. This password authorizes recovery tools.")).tone(TextTone::Muted));
                    if self.accounts.is_empty() {
                        body=body.push(enz_text(crate::i18n::label("No administrator account is available. Use command-line recovery to inspect the account configuration.")));
                    }
                }
                Page::Files => {
                    body = body.push(self.file_manager());
                }
                Page::Snapshots => {
                    if let Some(choices) = self.state["choices"].as_array() {
                        if choices.is_empty() {
                            body = body.push(
                                enz_text(crate::i18n::label(
                                    "No bootable system snapshots are available yet.",
                                ))
                                .tone(TextTone::Muted),
                            );
                        }
                        for (i, c) in choices.iter().enumerate() {
                            body = body.push(
                                row![
                                    enz_icon("history").size(26.0),
                                    column![
                                        enz_text(snapshot_label(c)).size(TextSize::Title3),
                                        enz_text(format!(
                                            "{} · {}",
                                            c["created"]
                                                .as_str()
                                                .unwrap_or(crate::i18n::label("Date unavailable")),
                                            c["kernels"]
                                                .as_array()
                                                .map(|v| v
                                                    .iter()
                                                    .filter_map(Value::as_str)
                                                    .collect::<Vec<_>>()
                                                    .join(" · "))
                                                .unwrap_or_default()
                                        ))
                                        .tone(TextTone::Muted),
                                        enz_text(if c["available"] == false {
                                            c["reason"]
                                                .as_str()
                                                .unwrap_or(crate::i18n::label("Unavailable"))
                                        } else {
                                            crate::i18n::label(
                                                "System files only · personal files kept",
                                            )
                                        })
                                        .tone(TextTone::Muted)
                                    ]
                                    .width(Length::Fill),
                                    enz_button(
                                        crate::i18n::label("Review snapshot"),
                                        Msg::ChooseSnapshot(i)
                                    )
                                    .enabled(
                                        c["available"] != false
                                            && self.state["images"]
                                                .as_array()
                                                .is_some_and(|v| !v.is_empty())
                                    )
                                    .into_element()
                                ]
                                .spacing(20)
                                .align_y(Alignment::Center),
                            );
                        }
                    }
                    if let Some(a) = self.state["activations"].as_object() {
                        for (id, v) in a {
                            if v["phase"] == "selected"
                                || (v["phase"] == "adopted"
                                    && self.state["runs"][id]["phase"] == "executed")
                            {
                                body = body.push(
                                    enz_button(
                                        crate::i18n::label("Confirm recovered system"),
                                        Msg::Finish(id.clone()),
                                    )
                                    .kind(ButtonKind::Accent)
                                    .into_element(),
                                );
                            }
                        }
                    }
                }

                Page::Backup => {
                    body = body.push(self.backup.view().map(Msg::Backup));
                }
                Page::Logs => {
                    body = body.push(
                        container(
                            enz_scroll_view(text(&self.logs).font(iced::Font::MONOSPACE).size(13))
                                .height(370)
                                .into_element(),
                        )
                        .padding(16),
                    );
                }
                Page::Power => {
                    body = body.push(
                        row![
                            self.tile(
                                "rotate-cw",
                                crate::i18n::label("Restart computer"),
                                crate::i18n::label("Return to the boot menu."),
                                Msg::Reboot
                            ),
                            self.tile(
                                "power",
                                crate::i18n::label("Shut down"),
                                crate::i18n::label("Turn off this computer."),
                                Msg::Shutdown
                            )
                        ]
                        .spacing(20),
                    );
                }
                Page::Terminal | Page::Finished => {}
            }
        }
        let mut footer = row![].spacing(12).align_y(Alignment::Center);

        footer = footer.push(Space::new().width(Length::Fill));
        if self.busy && self.action == "files_copy" {
            footer = footer.push(
                enz_button(crate::i18n::label("Cancel copy"), Msg::Cancel)
                    .enabled(!self.cancelling)
                    .into_element(),
            );
        } else {
            let action = match self.page {
                Page::Auth => Some((
                    crate::i18n::label("Unlock tools"),
                    Msg::Authenticate,
                    !self.password.is_empty() && !self.user.is_empty(),
                )),
                Page::Files => None,

                Page::Finished => Some((crate::i18n::label("Restart computer"), Msg::Reboot, true)),
                _ => None,
            };
            if let Some((label, msg, enabled)) = action {
                footer = footer.push(
                    enz_button(label, msg)
                        .kind(ButtonKind::Accent)
                        .enabled(enabled && !self.busy)
                        .into_element(),
                );
            }
        }
        if self.page == Page::Home && !self.busy {
            return enz_scroll_view(container(body).padding([30.0, 44.0]))
                .height(Length::Fill)
                .into_element();
        }
        if !matches!(self.page, Page::Auth | Page::Finished) {
            return enz_scroll_view(container(body).padding([30.0, 44.0]))
                .height(Length::Fill)
                .into_element();
        }
        column![
            enz_scroll_view(container(body).padding(if self.page == Page::Files {
                [18.0, 24.0]
            } else {
                [30.0, 44.0]
            }))
            .height(Length::Fill)
            .into_element(),
            enz_divider().into_element(),
            container(footer).padding([18.0, 24.0])
        ]
        .into()
    }
}
