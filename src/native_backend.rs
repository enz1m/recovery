// SPDX-License-Identifier: MIT
use crate::backend::Event;
use enzimos_recovery_transport::{Client, Message, Replies};
use iced::futures::{SinkExt, channel::mpsc::Sender};
use serde_json::{Value, json};
use std::time::Duration;
use zeroize::Zeroizing;
pub(crate) async fn run(
    request: Value,
    user: String,
    password: Zeroizing<String>,
    cancel: &mut tokio::sync::oneshot::Receiver<()>,
    output: &mut Sender<Event>,
) -> Result<Value, String> {
    let mut client = Client::spawn().map_err(|e| e.to_string())?;
    client
        .send_initial(&request, &user, password, false)
        .await
        .map_err(|e| e.to_string())?;
    let mut replies = Replies::new(request["action"] == "accounts");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(86400);
    let mut wait = Duration::from_secs(35);
    let mut result = None;
    let mut failure = None;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(crate::i18n::label(
                "Recovery operation wait limit reached. Inspect its result before trying again.",
            )
            .into());
        }
        let value = tokio::select! {
            _=&mut *cancel => {
                client.cancel().await.map_err(|e|e.to_string())?;
                return Err(crate::i18n::label("Recovery operation cancelled.").into());
            },
            next=client.next(wait.min(remaining)) => next.map_err(|e|e.to_string())?,
        };
        let Some(value) = value else {
            break;
        };
        match replies.accept(value).map_err(|e| e.to_string())? {
            Message::Ready => (),
            Message::Authenticated => wait = Duration::from_secs(86400),
            Message::Progress(value) => output
                .send(Event::Progress(value))
                .await
                .map_err(|_| crate::i18n::label("Recovery interface closed").to_owned())?,
            Message::Result(value) => {
                result = Some(value);
                wait = Duration::from_secs(20);
            }
            Message::Failure(error) => {
                failure = Some(error);
                wait = Duration::from_secs(20);
            }
        }
    }
    let exit = client.finish().await.map_err(|e| e.to_string());
    if let Some(error) = failure {
        return Err(error);
    }
    exit?;
    result.ok_or_else(|| crate::i18n::label("Recovery service returned no result.").into())
}
pub(crate) async fn browse(
    user: String,
    password: Zeroizing<String>,
    mut requests: tokio::sync::mpsc::UnboundedReceiver<Value>,
    output: &mut Sender<Event>,
) -> Result<(), String> {
    let mut client = Client::spawn().map_err(|e| e.to_string())?;
    client
        .send_initial(&json!({"action":"browse_session"}), &user, password, true)
        .await
        .map_err(|e| e.to_string())?;
    let mut replies = Replies::new(false);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(35);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(crate::i18n::label("Browsing authentication timed out.").into());
        }
        let message = client
            .next(remaining)
            .await
            .map_err(|e| e.to_string())?
            .ok_or(crate::i18n::label("Browsing authentication failed"))?;
        match replies.accept(message).map_err(|e| e.to_string())? {
            Message::Ready => (),
            Message::Authenticated => break,
            Message::Failure(error) => return Err(error),
            _ => {
                return Err(
                    crate::i18n::label("Unexpected browsing authentication response.").into(),
                );
            }
        }
    }
    while let Some(request) = requests.recv().await {
        let action = request["action"]
            .as_str()
            .ok_or(crate::i18n::label("Browsing action unavailable"))?
            .to_owned();
        client
            .send_browse(&request)
            .await
            .map_err(|e| e.to_string())?;
        let response = client
            .next(Duration::from_secs(60))
            .await
            .map_err(|e| e.to_string())?
            .ok_or(crate::i18n::label("Browsing connection closed"))?;
        if response["action"] != action {
            return Err(crate::i18n::label("Browsing response did not match the request.").into());
        }
        let result = if let Some(error) = response["error"].as_str() {
            Err(error.chars().take(500).collect())
        } else {
            if !response["result"].is_object() {
                return Err(crate::i18n::label("Invalid browsing result.").into());
            }
            Ok(response["result"].clone())
        };
        output
            .send(Event::Done(action, result))
            .await
            .map_err(|_| crate::i18n::label("Recovery interface closed").to_owned())?;
    }
    client.cancel().await.map_err(|e| e.to_string())
}
