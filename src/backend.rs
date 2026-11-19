use iced::futures::{SinkExt, Stream};
use serde_json::{Value, json};
use zeroize::Zeroizing;
#[derive(Debug, Clone)]
pub(crate) enum Event {
    Progress(Value),
    Done(String, Result<Value, String>),
}
pub(crate) fn run(
    request: Value,
    user: String,
    password: Zeroizing<String>,
    demo: bool,
    mut cancel: tokio::sync::oneshot::Receiver<()>,
) -> impl Stream<Item = Event> {
    iced::stream::channel(
        32,
        async move |mut out: iced::futures::channel::mpsc::Sender<Event>| {
            let action = request["action"].as_str().unwrap_or("").to_owned();
            let result = if demo {
                preview_result(&request)
            } else {
                crate::native_backend::run(request, user, password, &mut cancel, &mut out).await
            };
            let _ = out.send(Event::Done(action, result)).await;
        },
    )
}
/// The preview's answer. `ENZIMOS_RECOVERY_DEMO_FAIL=<action>` makes that
/// preview action fail, so captures can show the failure notice.
fn preview_result(request: &Value) -> Result<Value, String> {
    let action = request["action"].as_str().unwrap_or("");
    if std::env::var("ENZIMOS_RECOVERY_DEMO_FAIL").is_ok_and(|failing| failing == action) {
        return Err(crate::i18n::label("Recovery service returned no result.").into());
    }
    Ok(preview(request))
}
fn preview(request: &Value) -> Value {
    match request["action"].as_str().unwrap_or("") {
        "accounts" => {
            json!({"accounts":[{"name":"admin","label":crate::i18n::label("Administrator")}]})
        }
        "authenticate" => json!({"authenticated_user":"admin"}),
        "inspect" => {
            json!({"system":"enzimos","digest":"demo","choices":[{"birth":"demo","generation":"before-update","ordinal":1,"available":true,"created":"29 Sep 2026 · 12:00 UTC","kernels":["linux","linux-lts"]}],"images":["demo"],"activations":{},"runs":{}})
        }
        "files_list" => {
            json!({"path":request["path"],"page":0,"more":false,"entries":[{"name":"Documents","path":"/home/admin/Documents","directory":true,"size":0},{"name":"Pictures","path":"/home/admin/Pictures","directory":true,"size":0},{"name":"notes.txt","path":"/home/admin/notes.txt","directory":false,"size":1240}]})
        }
        "snapshot_details" => {
            json!({"created":"29 Sep 2026 · 12:00 UTC","description":"System restore point · linux, linux-lts","summary":"460 packages in snapshot · 8 version changes","changes":["linux   7.2 → 7.1","linux-lts   6.18.8 → 6.18.7","mesa   26.2 → 26.1","systemd   262 → 261","pipewire   1.6 → 1.5","networkmanager   1.56 → 1.54","bluez   5.90 → 5.89","openssl   3.6 → 3.5"]})
        }
        "files_destination" => {
            json!({"path":request["path"],"page":0,"more":false,"entries":if request["path"]=="" {json!([{"name":"Documents","path":"Documents","directory":true,"size":0}])}else{json!([])}})
        }
        "files_drives" => {
            json!({"drives":[{"id":"demo","label":crate::i18n::label("Personal backup"),"fstype":"exfat","size":64000000000_u64,"reason":""}]})
        }
        "files_plan" => {
            json!({"plan":"demo","digest":"demo","bytes":240000000,"files":84,"skipped":0,"destination":crate::i18n::label("Personal backup")})
        }
        "files_copy" => {
            json!({"copied":true,"folder":"enzimOS-backup-preview","files":84,"bytes":240000000,"skipped":0,"drive":"demo"})
        }
        "files_eject" => json!({"ejected":true}),
        "terminal" => json!({"terminal":true}),
        "logs" => {
            json!({"logs":"Preview diagnostics\nFilesystems mounted successfully.\nNo system changes were made."})
        }
        _ => {
            json!({"message":crate::i18n::label("Preview complete. No system changes were made.")})
        }
    }
}

// Keep one authenticated read-only native connection for navigation.
// Mutations use run() and its original cancellation lifecycle.
pub(crate) fn browse(
    user: String,
    password: Zeroizing<String>,
    demo: bool,
    mut requests: tokio::sync::mpsc::UnboundedReceiver<Value>,
) -> impl Stream<Item = Event> {
    iced::stream::channel(
        32,
        async move |mut out: iced::futures::channel::mpsc::Sender<Event>| {
            if demo {
                while let Some(request) = requests.recv().await {
                    let _ = out
                        .send(Event::Done(
                            request["action"].as_str().unwrap_or("").into(),
                            preview_result(&request),
                        ))
                        .await;
                }
                return;
            }
            if let Err(error) =
                crate::native_backend::browse(user, password, requests, &mut out).await
            {
                let _ = out
                    .send(Event::Done("browse_closed".into(), Err(error)))
                    .await;
            }
        },
    )
}
