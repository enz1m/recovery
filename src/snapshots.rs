use super::*;
use iced::widget::{column, row};
impl Recovery {
    pub(crate) fn snapshots_view(&self) -> Element<'_, Msg> {
        let mut list =
            column![enz_text(crate::i18n::label("Restore points")).size(TextSize::Title3)]
                .spacing(8);
        if let Some(choices) = self.state["choices"].as_array() {
            for (i, c) in choices.iter().enumerate() {
                let selected = self.snapshot == Some(i);
                let available = c["available"] != false;
                let content = row![
                    enz_icon(if selected { "circle-check" } else { "history" })
                        .size(22.0)
                        .tint_theme_accent(),
                    column![
                        enz_text(
                            c["created"]
                                .as_str()
                                .unwrap_or(crate::i18n::label("Date unavailable"))
                        )
                        .size(TextSize::Body),
                        enz_text(snapshot_label(c))
                            .size(TextSize::Caption)
                            .tone(TextTone::Muted)
                    ]
                    .spacing(5)
                ]
                .spacing(12)
                .align_y(Alignment::Center);
                let mut b = iced::widget::button(content)
                    .padding(14)
                    .width(Length::Fill)
                    .style(move |theme, status| {
                        let p = enzui_widgets::theme_lookup::palette(theme);
                        let hover = matches!(
                            status,
                            iced::widget::button::Status::Hovered
                                | iced::widget::button::Status::Pressed
                        );
                        iced::widget::button::Style {
                            background: Some(
                                if selected {
                                    p.surface_selected
                                } else if hover {
                                    p.surface_hover
                                } else {
                                    p.surface
                                }
                                .into(),
                            ),
                            text_color: p.text,
                            border: iced::Border {
                                radius: 9.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    });
                if available && !self.busy {
                    b = b.on_press(Msg::ChooseSnapshot(i));
                }
                list = list.push(b);
                if !available {
                    list = list.push(
                        enz_text(
                            c["reason"]
                                .as_str()
                                .unwrap_or(crate::i18n::label("Unavailable")),
                        )
                        .size(TextSize::Caption)
                        .tone(TextTone::Muted),
                    );
                }
            }
            if choices.is_empty() {
                list = list.push(
                    enz_text(crate::i18n::label("No restore points yet")).tone(TextTone::Muted),
                );
            }
        } else {
            list = list.push(
                enz_text(crate::i18n::label("Finding restore points…")).tone(TextTone::Muted),
            );
        }
        let mut detail = column![].spacing(20);
        if let Some(i) = self.snapshot {
            let c = &self.state["choices"][i];
            detail = detail
                .push(enz_text(crate::i18n::label("Selected restore point")).tone(TextTone::Muted))
                .push(
                    enz_text(
                        c["created"]
                            .as_str()
                            .unwrap_or(crate::i18n::label("System snapshot")),
                    )
                    .size(TextSize::Title2),
                );
            if self.busy && self.action == "snapshot_details" {
                detail = detail.push(
                    enz_text(crate::i18n::label("Reading system changes…")).tone(TextTone::Muted),
                );
            } else if !self.details.is_null() {
                detail = detail.push(self.snapshot_details());
            }
            detail=detail.push(enz_divider().into_element())
                .push(row![enz_icon("folder-check").size(22.0).tint_theme_accent(),column![
                    enz_text(crate::i18n::label("Your personal files stay as they are")).size(TextSize::Title3),
                    enz_text(crate::i18n::label("System files return to this restore point. Your home folder, disk encryption and package trust are preserved.")).tone(TextTone::Muted)].spacing(8)] .spacing(12))
                .push(enz_text(crate::i18n::label("The computer restarts, asks for your disk passphrase, and restores the system automatically. Keep it connected to power.")).tone(TextTone::Muted));
        } else {
            detail=detail.push(enz_icon("history").size(40.0).tint_theme_accent())
                .push(enz_text(crate::i18n::label("Choose a restore point")).size(TextSize::Title2))
                .push(enz_text(crate::i18n::label("See what will change before restoring. Your current system is retained so recovery can be reviewed if something goes wrong.")).tone(TextTone::Muted));
        }
        if !self.error.is_empty() {
            detail = detail.push(
                enz_banner(crate::i18n::label("Could not restore the system"))
                    .body(&self.error)
                    .tone(BannerTone::Danger)
                    .into_element(),
            );
        }
        if self.busy && self.action == "restore" {
            detail = detail.push(progress_bar(0.0..=1.0, self.loading).girth(4));
        }
        let ready = self.snapshot.is_some()
            && !self.details.is_null()
            && !self.busy
            && self.state["images"]
                .as_array()
                .is_some_and(|v| !v.is_empty());
        container(
            column![
                column![
                    enz_text(crate::i18n::label("Restore system")).size(TextSize::Title1),
                    enz_text(crate::i18n::label("Return to an earlier working state"))
                        .tone(TextTone::Muted)
                ]
                .spacing(8),
                row![
                    // Up to 340 px, sharing the width with the detail on a
                    // narrow window instead of leaving it a 30 px column.
                    container(enz_scroll_view(list).height(Length::Fill).into_element())
                        .width(Length::Fill.max(340.0)),
                    container(enz_scroll_view(detail).height(Length::Fill).into_element())
                        .width(Length::Fill)
                ]
                .spacing(30)
                .height(Length::Fill),
                row![
                    Space::new().width(Length::Fill),
                    enz_button(
                        if self.busy && self.action == "restore" {
                            crate::i18n::label("Starting recovery…")
                        } else {
                            crate::i18n::label("Restore and restart")
                        },
                        Msg::Restore
                    )
                    .icon("rotate-ccw")
                    .kind(ButtonKind::Accent)
                    .enabled(ready)
                    .into_element()
                ]
                .align_y(Alignment::Center)
            ]
            .spacing(26)
            .height(Length::Fill),
        )
        .padding([28, 32])
        .height(Length::Fill)
        .into()
    }
    pub(crate) fn logs_view(&self) -> Element<'_, Msg> {
        container(
            column![
                row![
                    column![
                        enz_text(crate::i18n::label("Boot logs")).size(TextSize::Title1),
                        enz_text(crate::i18n::label("Current boot · refreshes automatically"))
                            .tone(TextTone::Muted)
                    ]
                    .spacing(8),
                    Space::new().width(Length::Fill),
                    enz_button(crate::i18n::label("Copy with QR"), Msg::ShowQr)
                        .icon("qr-code")
                        .kind(ButtonKind::Secondary)
                        .enabled(!self.logs.is_empty())
                        .into_element()
                ]
                .align_y(Alignment::Center),
                enz_scroll_view(
                    container(text(&self.logs).font(iced::Font::MONOSPACE).size(13)).padding(16)
                )
                .height(Length::Fill)
                .into_element(),
            ]
            .spacing(22)
            .height(Length::Fill),
        )
        .padding([28, 32])
        .height(Length::Fill)
        .into()
    }
    pub(crate) fn qr_dialog<'a>(&'a self, q: &'a log_transfer::Transfer) -> Element<'a, Msg> {
        enzui_widgets::modal_surface::enz_modal_surface(
            column![
                row![
                    enz_text(crate::i18n::label("Latest boot output")).size(TextSize::Title2),
                    Space::new().width(Length::Fill),
                    enz_button(crate::i18n::label("Close"), Msg::CloseQr)
                        .kind(ButtonKind::Secondary)
                        .into_element()
                ]
                .align_y(Alignment::Center),
                container(iced::widget::qr_code(&q.code).total_size(360.0).style(|_| {
                    iced::widget::qr_code::Style {
                        cell: iced::Color::BLACK,
                        background: iced::Color::WHITE,
                    }
                }))
                .padding(16)
                .style(|_| container::Style::default().background(iced::Color::WHITE))
                .center_x(Length::Fill),
                enz_text(crate::i18n::label("Scan to copy · updates every second"))
                    .size(TextSize::Caption)
                    .tone(TextTone::Muted)
            ]
            .spacing(16)
            .into(),
        )
        .width(540.0)
        .padding(20.0)
        .into_element()
    }
}
