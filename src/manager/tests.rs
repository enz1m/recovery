use super::*;
fn app() -> Recovery {
    let (mut app, _) = Recovery::new(true);
    app.busy = false;
    app.authenticated = true;
    app.page = Page::Files;
    app.drive = "demo".into();
    app.browser.set_entries(vec![
        FileEntry::file("/home/admin/a", "a", 1),
        FileEntry::file("/home/admin/b", "b", 2),
    ]);
    app.refresh_views();
    app
}
#[test]
fn deletion_requires_explicit_confirmation() {
    let mut a = app();
    a.selected.insert("/home/admin/a".into());
    let _ = a.manager_update(Event::Action(false, "delete".into()));
    assert!(a.fm.dialog.is_some());
    assert!(!a.busy);
    let _ = a.manager_update(Event::Dismiss);
    assert!(!a.busy);
    assert!(a.fm.dialog.is_none());
    let _ = a.manager_update(Event::Action(false, "delete".into()));
    let _ = a.manager_update(Event::Confirm);
    assert!(a.busy);
    assert_eq!(a.action, "files_manager_plan");
    assert_eq!(a.fm.operation, "delete");
}
#[test]
fn clipboard_remembers_source_panel() {
    let mut a = app();
    a.selected.insert("/home/admin/a".into());
    let _ = a.manager_update(Event::Action(false, "copy".into()));
    assert_eq!(a.fm.clipboard.as_ref().unwrap().0, json!({}));
    let _ = a.manager_update(Event::Activate(true));
    assert!(a.fm.active);
    let _ = a.manager_update(Event::Action(true, "paste".into()));
    assert!(a.busy);
    assert!(a.fm.operation_right);
    assert_eq!(a.fm.operation, "copy");
}
#[test]
fn ordinary_click_replaces_selection_and_shift_extends() {
    let mut a = app();
    let _ = a.manager_update(Event::Select(
        false,
        "/home/admin/a".into(),
        iced::keyboard::Modifiers::default(),
    ));
    let _ = a.manager_update(Event::Select(
        false,
        "/home/admin/b".into(),
        iced::keyboard::Modifiers::SHIFT,
    ));
    assert_eq!(a.selected.len(), 2);
    let _ = a.manager_update(Event::Select(
        false,
        "/home/admin/a".into(),
        iced::keyboard::Modifiers::default(),
    ));
    assert_eq!(a.selected.len(), 1);
}
#[test]
fn cached_navigation_is_visible_while_refreshing() {
    let mut a = app();
    let v = json!({"path":"/home/admin/Documents","entries":[{"path":"/home/admin/Documents/saved","name":"saved","size":1,"directory":false}]});
    a.fm.remember(false, &v, "");
    let _ = a.navigate(false, "/home/admin/Documents".into());
    assert!(a.busy);
    assert!(a.browsing());
    assert_eq!(a.browser.visible_len(), 1);
    assert_eq!(a.fm.rows[0].0[0].name, "saved");
    assert_eq!(a.page, Page::Files);
}
