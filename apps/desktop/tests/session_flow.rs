//! End-to-end flows over a file-backed store: restore roundtrips,
//! multi-window sessions, the IPC report pipeline, and corruption fallback.

use phantom_desktop::browser::{Effect, PhantomSession};
use phantom_desktop::domain::command::{BrowserCommand, NavigationTarget};
use phantom_desktop::domain::layout::{PaneId, SplitDirection};
use phantom_desktop::domain::settings::{FeatureFlag, Settings};
use phantom_desktop::domain::space::SpaceId;
use phantom_desktop::domain::tab::TabId;
use phantom_desktop::infra::db::{SessionSnapshot, Store, WindowSnapshot};
use phantom_desktop::infra::ipc::{decode_webview_message, WebviewToHost};
use phantom_desktop::infra::paths::database_path;

fn focused_window(session: &PhantomSession) -> u64 {
    session.focused_window_id().expect("a focused window")
}

fn focused_tab(session: &PhantomSession) -> TabId {
    session
        .window(focused_window(session))
        .and_then(|window| window.active_tab_id())
        .expect("a focused tab")
}

fn focused_pane(session: &PhantomSession) -> PaneId {
    session
        .window(focused_window(session))
        .map(|window| window.focused_pane)
        .expect("a focused pane")
}

#[test]
fn full_session_roundtrips_through_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = database_path(dir.path());

    let mut first = PhantomSession::new(Store::open(&path).unwrap(), Settings::default());
    first.ensure_startup_window().unwrap();
    let startup_tab = focused_tab(&first);
    first
        .handle(BrowserCommand::Navigate {
            tab: startup_tab,
            target: NavigationTarget::Url(url::Url::parse("https://example.com/").unwrap()),
        })
        .unwrap();
    first
        .handle(BrowserCommand::ToggleFlag {
            flag: FeatureFlag::EnableTiling,
            enabled: true,
        })
        .unwrap();
    let split_effects = first
        .handle(BrowserCommand::SplitPane {
            pane: focused_pane(&first),
            direction: SplitDirection::Horizontal,
            target: NavigationTarget::NewTabPage,
        })
        .unwrap();
    assert_eq!(split_effects.len(), 1, "attach for the split tab");
    first
        .handle(BrowserCommand::NewWindow {
            space: SpaceId::default_space(),
        })
        .unwrap();
    first.flush().unwrap();
    drop(first);

    let mut second = PhantomSession::new(Store::open(&path).unwrap(), Settings::default());
    let effects = second.restore().unwrap();
    // OpenWindow per window (2) + AttachWebview per visible pane tab (2 + 1)
    assert_eq!(effects.len(), 5, "{effects:?}");
    assert!(effects.iter().all(|effect| matches!(
        effect,
        Effect::OpenWindow { .. } | Effect::AttachWebview { .. }
    )));

    assert_eq!(second.windows().len(), 2);
    let restored = second.window(1).unwrap();
    assert_eq!(restored.pane_count(), 2);
    assert_eq!(restored.tabs.len(), 2);
    assert_eq!(
        restored.tabs[0].url.as_ref().map(|url| url.as_str()),
        Some("https://example.com/")
    );
    assert_eq!(restored.tabs[0].title, "example.com");

    // id allocation continues past the restored maximum (tab 3 was the last)
    second
        .handle(BrowserCommand::NewTab {
            space: SpaceId::default_space(),
            target: NavigationTarget::NewTabPage,
        })
        .unwrap();
    let new_tab = second
        .window(1)
        .and_then(|window| window.active_tab_id())
        .unwrap();
    assert_eq!(new_tab.0, 4);
}

#[test]
fn corrupt_window_snapshot_is_skipped_for_a_clean_start() {
    let mut store = Store::open_in_memory().unwrap();
    let bad = WindowSnapshot {
        id: 1,
        bounds: Default::default(),
        maximized: false,
        active_tab: Some(TabId(9)),
        layout: phantom_desktop::domain::layout::LayoutTree::pane(PaneId(1), TabId(9)),
        tabs: Vec::new(),
    };
    store
        .save_session(&SessionSnapshot { windows: vec![bad] })
        .unwrap();
    let mut session = PhantomSession::new(store, Settings::default());
    let effects = session.restore().unwrap();
    assert!(effects.is_empty());
    assert!(session.windows().is_empty());
    // a browser must still launch after losing its stale snapshot
    session.ensure_startup_window().unwrap();
    assert_eq!(session.windows().len(), 1);
}

#[test]
fn ipc_reports_flow_into_session_state() {
    let mut session = PhantomSession::new(Store::open_in_memory().unwrap(), Settings::default());
    session.ensure_startup_window().unwrap();
    let tab = focused_tab(&session);

    let raw = "{\"type\":\"navigation_state_changed\",\"state\":{\"url\":\"https://example.com/\",\"title\":\"Example Domain\",\"can_go_back\":false,\"can_go_forward\":false,\"is_loading\":false}}";
    match decode_webview_message(raw).unwrap() {
        WebviewToHost::NavigationStateChanged { state } => {
            let effects = session.report_navigation(tab, state).unwrap();
            assert!(matches!(effects[0], Effect::SetWindowTitle { .. }));
        }
        other => panic!("wrong variant: {other:?}"),
    }
    let window = session.window(focused_window(&session)).unwrap();
    let loaded = window.tab(tab).unwrap();
    assert_eq!(loaded.title, "Example Domain");
    assert_eq!(
        loaded.status,
        phantom_desktop::domain::tab::TabStatus::Loaded
    );

    // a hostile message is refused before it can touch the session
    let hostile =
        "{\"type\":\"open_in_new_tab\",\"url\":\"javascript:alert(1)\",\"background\":false}";
    assert!(decode_webview_message(hostile).is_err());

    // a legit open request opens a tab in the same space
    let open =
        "{\"type\":\"open_in_new_tab\",\"url\":\"https://example.org/\",\"background\":false}";
    match decode_webview_message(open).unwrap() {
        WebviewToHost::OpenInNewTab { url, background } => {
            let effects = session.report_open_request(tab, url, background).unwrap();
            assert_eq!(effects.len(), 1, "surface attached for foreground open");
        }
        other => panic!("wrong variant: {other:?}"),
    }
    let window = session.window(focused_window(&session)).unwrap();
    assert_eq!(window.tabs.len(), 2);
    let opened = window.tabs.last().unwrap();
    assert_eq!(opened.space.as_str(), "default");
}

#[test]
fn quit_persists_an_empty_session_and_ids_start_over() {
    let dir = tempfile::tempdir().unwrap();
    let path = database_path(dir.path());

    let mut session = PhantomSession::new(Store::open(&path).unwrap(), Settings::default());
    session.ensure_startup_window().unwrap();
    session.quit().unwrap();
    drop(session);

    let mut next = PhantomSession::new(Store::open(&path).unwrap(), Settings::default());
    assert!(next.restore().unwrap().is_empty());
    next.ensure_startup_window().unwrap();
    // fresh session: allocation restarts at 1
    assert_eq!(focused_tab(&next).0, 1);
}

#[test]
fn dismiss_window_only_leaves_the_other_one() {
    let mut session = PhantomSession::new(Store::open_in_memory().unwrap(), Settings::default());
    session.ensure_startup_window().unwrap();
    session
        .handle(BrowserCommand::NewWindow {
            space: SpaceId::default_space(),
        })
        .unwrap();
    assert_eq!(session.windows().len(), 2);
    let effects = session.dismiss_window(1).unwrap();
    assert_eq!(
        effects.len(),
        1,
        "detach effect for the dismissed window's tab"
    );
    assert_eq!(session.windows().len(), 1);
    assert_eq!(focused_window(&session), 2);
}
