//! Application bootstrap: builds the session from persisted state, opens the
//! restored windows, installs the shared shell state, and keeps a light
//! pump running so webview reports render even without user input.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use gpui::{
    px, App, AppContext, Application, AsyncApp, Bounds, Global, Point, Size, TitlebarOptions,
    WindowBounds, WindowOptions,
};

use crate::browser::{PhantomSession, WindowId};
use crate::domain::settings::Settings;
use crate::infra::db::{Store, WindowBounds as StoredBounds};
use crate::infra::paths;
use crate::infra::settings_store;
use crate::ui::actions;
use crate::ui::settings_window::SettingsView;
use crate::ui::window::BrowserWindow;
use crate::webview::host::HostEvent;

/// State shared by every window of a running instance. The session sits
/// behind a mutex because rusqlite's connection is Send, not Sync, while
/// GPUI globals require Send + Sync.
pub struct BrowserShell {
    pub session: Mutex<PhantomSession>,
    pub profile: PathBuf,
    pub events_tx: Sender<HostEvent>,
    pub events_rx: Mutex<Receiver<HostEvent>>,
    pub settings_queue: Arc<Mutex<Vec<crate::domain::command::BrowserCommand>>>,
}

impl Global for BrowserShell {}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Applies pending webview reports to the session.
pub(crate) fn drain_host_events(cx: &App) {
    let shell = cx.global::<BrowserShell>();
    loop {
        let event = match lock(&shell.events_rx).try_recv() {
            Ok(event) => event,
            Err(_) => break,
        };
        let mut session = lock(&shell.session);
        match event {
            HostEvent::NavigationStateChanged { tab, state } => {
                let _ = session.report_navigation(tab, state);
            }
            HostEvent::OpenInNewTab {
                tab,
                url,
                background,
            } => {
                let _ = session.report_open_request(tab, url, background);
            }
        }
    }
}

/// Applies commands queued by the settings window.
pub(crate) fn drain_settings_queue(cx: &App) {
    let shell = cx.global::<BrowserShell>();
    let pending = {
        let mut queue = lock(&shell.settings_queue);
        std::mem::take(&mut *queue)
    };
    if pending.is_empty() {
        return;
    }
    let mut session = lock(&shell.session);
    for command in pending {
        let _ = session.handle(command);
    }
}

/// Opens one browser window for a session window id.
pub(crate) fn open_browser_window(
    cx: &mut App,
    window_id: WindowId,
    bounds: Option<StoredBounds>,
    maximized: bool,
    profile: PathBuf,
) {
    let window_bounds = restored_bounds(bounds, maximized, cx);
    let options = WindowOptions {
        window_bounds: Some(window_bounds),
        titlebar: Some(TitlebarOptions {
            title: Some("Phantom".into()),
            appears_transparent: true,
            traffic_light_position: None,
        }),
        ..Default::default()
    };
    let _ = cx.open_window(options, |_window, cx| {
        cx.new(|cx| BrowserWindow::new(window_id, profile, cx))
    });
}

fn restored_bounds(stored: Option<StoredBounds>, maximized: bool, cx: &App) -> WindowBounds {
    let bounds =
        stored.and_then(
            |stored| match (stored.x, stored.y, stored.width, stored.height) {
                (Some(x), Some(y), Some(width), Some(height)) if width > 0.0 && height > 0.0 => {
                    Some(Bounds {
                        origin: Point {
                            x: px(x as f32),
                            y: px(y as f32),
                        },
                        size: Size {
                            width: px(width as f32),
                            height: px(height as f32),
                        },
                    })
                }
                _ => None,
            },
        );
    match (bounds, maximized) {
        (Some(bounds), true) => WindowBounds::Maximized(bounds),
        (Some(bounds), false) => WindowBounds::Windowed(bounds),
        (None, true) => WindowBounds::Maximized(
            WindowBounds::centered(
                Size {
                    width: px(1200.0),
                    height: px(800.0),
                },
                cx,
            )
            .get_bounds(),
        ),
        (None, false) => WindowBounds::centered(
            Size {
                width: px(1200.0),
                height: px(800.0),
            },
            cx,
        ),
    }
}

/// Opens the native settings window.
pub(crate) fn open_settings_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            Size {
                width: px(720.0),
                height: px(560.0),
            },
            cx,
        )),
        titlebar: Some(TitlebarOptions {
            title: Some("Phantom Settings".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    };
    let _ = cx.open_window(options, |_window, cx| cx.new(SettingsView::new));
}

/// Entry point of the desktop shell.
pub fn launch(profile: PathBuf) {
    let db_path =
        paths::ensure_profile_dirs(&profile).expect("phantom: cannot create the profile directory");
    let store = Store::open(&db_path).expect("phantom: cannot open the state database");
    let settings = settings_store::load_or_default(&store).unwrap_or_else(|_| Settings::default());
    let mut session = PhantomSession::new(store, settings);
    // a corrupt snapshot must never keep the browser from launching
    let _ = session.restore();
    let _ = session.ensure_startup_window();
    let windows_to_open: Vec<(WindowId, Option<StoredBounds>, bool)> = session
        .windows()
        .iter()
        .map(|window| (window.id, Some(window.bounds), window.maximized))
        .collect();

    Application::new().run(move |cx: &mut App| {
        actions::bind_keys(cx);
        for (window_id, bounds, maximized) in windows_to_open {
            open_browser_window(cx, window_id, bounds, maximized, profile.clone());
        }
        let (events_tx, events_rx) = std::sync::mpsc::channel();
        cx.set_global(BrowserShell {
            session: Mutex::new(session),
            profile,
            events_tx,
            events_rx: Mutex::new(events_rx),
            settings_queue: Arc::new(Mutex::new(Vec::new())),
        });

        // light pump: keeps webview reports and settings updates rendering
        // even when nothing else wakes the frame loop
        cx.spawn(async move |cx: &mut AsyncApp| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let mut worked = false;
            let _ = cx.update(|cx| {
                let before = pending_counts(cx);
                drain_host_events(cx);
                drain_settings_queue(cx);
                let after = pending_counts(cx);
                worked = before != after;
            });
            if worked {
                let _ = cx.update(|cx| cx.refresh_windows());
            }
        })
        .detach();
    });
}

fn pending_counts(cx: &App) -> (usize, usize) {
    let shell = cx.global::<BrowserShell>();
    let events = lock(&shell.events_rx).try_iter().count();
    let queued = lock(&shell.settings_queue).len();
    (events, queued)
}
