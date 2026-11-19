// SPDX-License-Identifier: MIT
use super::{Client, Message, Replies, frame};
use serde_json::json;
use std::time::Duration;
use zeroize::Zeroizing;

fn fixture(script: &str) -> enzimos_platform::Result<Client> {
    let mut command = tokio::process::Command::new("/usr/bin/bash");
    command.args(["-c", script]);
    Client::spawn_command(command)
}

#[test]
fn password_frame_serializes_directly_into_one_bounded_private_buffer() {
    let request = json!({"action":"authenticate"});
    let encoded = frame(&request, "admin", "fixture\\\"secret").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value["password"], "fixture\\\"secret");
    assert_eq!(value["request"], request);
    assert!(
        frame(
            &json!({"action":"files_list","path":"x".repeat(65536)}),
            "admin",
            "fixture"
        )
        .is_err()
    );
    assert!(frame(&request, "-foreign", "fixture").is_err());
    assert!(frame(&request, "admin", "bad\npassword").is_err());
}
#[tokio::test]
async fn actual_owned_pipe_refuses_duplicate_and_truncated_responses() {
    for script in [
        "printf '%s\\n' '{\"error\":\"one\",\"error\":\"two\"}'",
        "printf '%s' '{\"partial\":true}'",
    ] {
        let mut child = fixture(script).unwrap();
        assert!(child.next(Duration::from_secs(5)).await.is_err());
        child.cancel().await.unwrap();
    }
    let mut child = fixture("IFS= read -r line; printf '%s\\n' '{\"authenticated\":true}' '{\"authenticated_user\":\"admin\"}'").unwrap();
    child
        .send_initial(
            &json!({"action":"authenticate"}),
            "admin",
            Zeroizing::new("fixture-secret".into()),
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        child.next(Duration::from_secs(5)).await.unwrap(),
        Some(json!({"authenticated":true}))
    );
    assert_eq!(
        child.next(Duration::from_secs(5)).await.unwrap(),
        Some(json!({"authenticated_user":"admin"}))
    );
    assert!(child.next(Duration::from_secs(5)).await.unwrap().is_none());
    child.finish().await.unwrap();
}
#[test]
fn recovery_replies_require_one_ordered_handoff_and_exactly_one_terminal_result() {
    let mut replies = Replies::new(false);
    assert!(replies.accept(json!({"authenticated":true})).is_err());
    assert!(
        replies
            .accept(json!({"authenticated_user":"admin"}))
            .is_err()
    );
    assert!(matches!(
        replies.accept(json!({"ready":true})).unwrap(),
        Message::Ready
    ));
    assert!(replies.accept(json!({"ready":true})).is_err());
    assert!(matches!(
        replies.accept(json!({"authenticated":true})).unwrap(),
        Message::Authenticated
    ));
    assert!(replies.accept(json!({"progress":false})).is_err());
    assert!(matches!(
        replies
            .accept(json!({"authenticated_user":"admin"}))
            .unwrap(),
        Message::Result(_)
    ));
    assert!(
        replies
            .accept(json!({"authenticated_user":"other"}))
            .is_err()
    );
    let mut failure = Replies::new(false);
    assert!(matches!(
        failure
            .accept(json!({"error":"Authentication refused"}))
            .unwrap(),
        Message::Failure(_)
    ));
    assert!(failure.accept(json!({"ready":true})).is_err());
}
#[tokio::test]
async fn browse_transport_denies_mutations_and_cancellation_reaps_only_its_child() {
    let mut child = fixture("IFS= read -r initial; IFS= read -r request; printf '%s\\n' '{\"action\":\"files_list\",\"result\":{\"entries\":[]}}'").unwrap();
    child
        .send_initial(
            &json!({"action":"browse_session"}),
            "admin",
            Zeroizing::new("fixture-secret".into()),
            true,
        )
        .await
        .unwrap();
    assert!(
        child
            .send_browse(&json!({"action":"files_copy"}))
            .await
            .is_err()
    );
    child
        .send_browse(&json!({"action":"files_list","path":"/home"}))
        .await
        .unwrap();
    assert_eq!(
        child.next(Duration::from_secs(5)).await.unwrap().unwrap()["action"],
        "files_list"
    );
    child.finish().await.unwrap();
    let mut idle = fixture("exec /usr/bin/sleep 60").unwrap();
    idle.cancel().await.unwrap();
    assert!(idle.finish().await.is_err());
}
#[tokio::test]
async fn response_reader_accepts_large_file_panels_and_stops_an_oversized_live_writer() {
    let mut child =
        fixture("printf -v body '%*s' 262144 ''; printf '{\"padding\":\"%s\"}\\n' \"$body\"")
            .unwrap();
    let response = child.next(Duration::from_secs(5)).await.unwrap().unwrap();
    assert_eq!(response["padding"].as_str().unwrap().len(), 262144);
    assert!(child.next(Duration::from_secs(5)).await.unwrap().is_none());
    child.finish().await.unwrap();
    let mut oversized =
        fixture("printf -v body '%*s' 33554434 ''; printf '%s\\n' \"$body\"").unwrap();
    assert!(oversized.next(Duration::from_secs(10)).await.is_err());
    oversized.cancel().await.unwrap();
}
