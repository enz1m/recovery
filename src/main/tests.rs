use super::*;
#[test]
fn password_events_are_redacted_and_account_changes_retire_browse_credentials() {
    let event = password_message("fixture-private-value".into());
    assert!(!format!("{event:?}").contains("fixture-private-value"));
    let (mut state, _) = Recovery::new(true);
    state.user = "first-admin".into();
    let _ = state.update(event);
    state.authenticated = true;
    state.revealed = true;
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    state.fm.browse = Some(sender);
    let _ = state.update(Msg::User("second-admin".into()));
    assert!(state.password.is_empty());
    assert!(!state.authenticated && !state.revealed && state.fm.browse.is_none());
}
#[test]
fn home_actions_require_authentication() {
    let (mut s, _) = Recovery::new(true);
    s.busy = false;
    let _ = s.update(Msg::Go(Page::Files));
    assert_eq!(s.page, Page::Auth);
    assert!(!s.authenticated);
    let _ = s.update(Msg::Back);
    let _ = s.update(Msg::Terminal);
    assert_eq!(s.page, Page::Auth);
    assert!(s.terminal_after_auth);
}
#[test]
fn no_implicit_restore_or_copy() {
    let (mut s, _) = Recovery::new(true);
    s.busy = false;
    let _ = s.update(Msg::Restore);
    let _ = s.update(Msg::Copy);
    assert!(!s.busy);
    s.page = Page::Snapshots;
    s.busy = true;
    let _ = s.update(Msg::Window(WindowCommand::Close));
    assert_eq!(s.page, Page::Snapshots);
    assert!(s.busy);
    s.busy = false;
    let _ = s.update(Msg::Window(WindowCommand::Close));
    assert_eq!(s.page, Page::Power);
    assert_eq!(s.power_return, Page::Snapshots);
}
#[test]
fn preview_never_reboots() {
    let (mut s, _) = Recovery::new(true);
    s.busy = false;
    let _ = s.update(Msg::Reboot);
    let _ = s.update(Msg::Shutdown);
    assert!(s.demo);
    s.error = "failed".into();
    s.notice = "done".into();
    let _ = s.update(Msg::DismissError);
    let _ = s.update(Msg::DismissNotice);
    assert!(s.error.is_empty() && s.notice.is_empty());
}
