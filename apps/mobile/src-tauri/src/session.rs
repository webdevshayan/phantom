//! Owns the browser session for one Space: the tab stack, the
//! preferences, the developer tools feed and every persisted side
//! effect. Pure logic — anything that must reach the webview comes back
//! as an Outcome the runtime layer interprets, so the whole state machine
//! stays testable without a webview. (Port of the old browser
//! controller.)

use crate::adblock;
use crate::devtools::DevToolsLogStore;
use crate::form::FormSnapshot;
use crate::omnibox::resolve_omnibox_input;
use crate::prefs::{Preferences, AD_BLOCKER_KEY};
use crate::restoration::SpaceStore;
use crate::search::{by_id, SearchEngine, SearchEngineId};
use crate::space::Space;
use crate::tab::{TabEntry, TabStack, BLANK_URL};
use crate::theme::ThemePreference;
use crate::wire::{
    DevToolsSnapshot, MenuState, PrefsState, TabStripItem, UiSnapshot, OVERLAY_NONE,
};

/// What the runtime must do with the webview after a session call.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LoadRequest {
    #[default]
    None,
    Url(String),
    Reload,
}

/// Side effects a session call asks the runtime to perform. Model and
/// persistence changes are already applied when the outcome is returned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// A new snapshot is waiting; the chrome must be woken.
    pub changed: bool,
    pub load: LoadRequest,
    /// Saved drafts to re-apply to the freshly loaded page.
    pub restore_form: Option<Vec<(String, String)>>,
}

impl Outcome {
    pub fn changed() -> Self {
        Self {
            changed: true,
            ..Self::default()
        }
    }

    pub fn with_load(load: LoadRequest) -> Self {
        Self {
            changed: true,
            load,
            restore_form: None,
        }
    }

    pub fn nothing() -> Self {
        Self::default()
    }
}

pub struct SessionState {
    space: Space,
    store: SpaceStore,
    stack: TabStack,
    prefs: Preferences,
    id_sequence: u64,
    suppress_url_events: bool,
    split_active: bool,
    split_tab_id: Option<String>,
    devtools: DevToolsLogStore,
    platform_dark: bool,
    overlay: u8,
    revision: u64,
}

impl SessionState {
    /// A fresh session for the Space.
    pub fn new(space: Space, store: SpaceStore) -> Self {
        let stack = TabStack::initial(&space.id);
        Self {
            space,
            store,
            stack,
            prefs: Preferences::INITIAL,
            id_sequence: 0,
            suppress_url_events: false,
            split_active: false,
            split_tab_id: None,
            devtools: DevToolsLogStore::new(),
            platform_dark: false,
            overlay: OVERLAY_NONE,
            revision: 0,
        }
    }

    /// Restores the session from the Space's own directory and asks for
    /// the active tab to load when it holds a real location.
    pub fn restore(&mut self) -> Outcome {
        self.prefs = self
            .store
            .load_preferences()
            .unwrap_or(Preferences::INITIAL);
        adblock::set_enabled(self.prefs.flags.enable_ad_blocker);
        let stack = self
            .store
            .load_tab_stack()
            .unwrap_or_else(|| TabStack::initial(&self.space.id));
        self.stack = if stack.tabs.is_empty() {
            TabStack::initial(&self.space.id)
        } else {
            stack
        };
        self.bump();
        let url = self.stack.active().url().to_string();
        let load = if url != BLANK_URL {
            LoadRequest::Url(url)
        } else {
            LoadRequest::None
        };
        Outcome::with_load(load)
    }

    /// Resolves the omnibox input and drives the active tab to the
    /// target. Empty input lands on the engine home, matching the old
    /// resolver.
    pub fn navigate(&mut self, raw_input: &str) -> Outcome {
        let engine = self.engine();
        let target = resolve_omnibox_input(raw_input, engine);
        let url = target.url.clone();
        let entry = self.stack.active().navigate_to(&url);
        self.replace_tab(entry);
        self.persist_stack();
        Outcome::with_load(LoadRequest::Url(url))
    }

    pub fn back(&mut self) -> Outcome {
        if !self.stack.active().can_go_back() {
            return Outcome::nothing();
        }
        let previous = self.stack.active().go_back();
        let url = previous.url().to_string();
        self.replace_tab(previous);
        self.suppress_url_events = true;
        self.persist_stack();
        Outcome::with_load(LoadRequest::Url(url))
    }

    pub fn forward(&mut self) -> Outcome {
        if !self.stack.active().can_go_forward() {
            return Outcome::nothing();
        }
        let next = self.stack.active().go_forward();
        let url = next.url().to_string();
        self.replace_tab(next);
        self.suppress_url_events = true;
        self.persist_stack();
        Outcome::with_load(LoadRequest::Url(url))
    }

    pub fn reload(&mut self) -> Outcome {
        Outcome {
            changed: false,
            load: LoadRequest::Reload,
            restore_form: None,
        }
    }

    pub fn add_tab(&mut self) -> Outcome {
        let entry = TabEntry::blank(self.next_tab_id(), self.engine().home_url);
        let url = entry.url().to_string();
        let id = entry.id.clone();
        let mut tabs = self.stack.tabs.clone();
        tabs.push(entry);
        self.stack = self.stack.with_tabs(tabs, Some(&id));
        self.persist_stack();
        Outcome::with_load(LoadRequest::Url(url))
    }

    pub fn select_tab(&mut self, tab_id: &str) -> Outcome {
        if self.stack.by_id(tab_id).is_none() {
            return Outcome::nothing();
        }
        self.stack = self.stack.with_tabs(self.stack.tabs.clone(), Some(tab_id));
        let url = self.stack.active().url().to_string();
        self.persist_stack();
        Outcome::with_load(LoadRequest::Url(url))
    }

    pub fn close_tab(&mut self, tab_id: &str) -> Outcome {
        if self.stack.by_id(tab_id).is_none() {
            return Outcome::nothing();
        }
        let was_active = tab_id == self.stack.active_tab_id;
        let remaining: Vec<TabEntry> = self
            .stack
            .tabs
            .iter()
            .filter(|tab| tab.id != tab_id)
            .cloned()
            .collect();
        if remaining.is_empty() {
            let fresh = TabEntry::blank(self.next_tab_id(), self.engine().home_url);
            let url = fresh.url().to_string();
            self.stack = TabStack {
                space_id: self.stack.space_id.clone(),
                tabs: vec![fresh.clone()],
                active_tab_id: fresh.id,
            };
            if self.split_tab_id.as_deref() == Some(tab_id) {
                self.split_tab_id = None;
                self.split_active = false;
            }
            self.persist_stack();
            return Outcome::with_load(LoadRequest::Url(url));
        }
        let mut next_active_id = self.stack.active_tab_id.clone();
        if was_active {
            let index = self
                .stack
                .tabs
                .iter()
                .position(|tab| tab.id == tab_id)
                .unwrap_or(0);
            let pick = index.min(remaining.len() - 1);
            next_active_id = remaining[pick].id.clone();
        }
        if self.split_tab_id.as_deref() == Some(tab_id) {
            self.split_tab_id = None;
            self.split_active = false;
        }
        self.stack = self.stack.with_tabs(remaining, Some(&next_active_id));
        let url = if was_active {
            self.stack.active().url().to_string()
        } else {
            String::new()
        };
        self.persist_stack();
        if was_active {
            Outcome::with_load(LoadRequest::Url(url))
        } else {
            Outcome::changed()
        }
    }

    pub fn reorder_tabs(&mut self, old_index: usize, new_index: usize) -> Outcome {
        if old_index >= self.stack.tabs.len() || old_index == new_index {
            return Outcome::nothing();
        }
        let mut tabs = self.stack.tabs.clone();
        let adjusted = if new_index > old_index {
            new_index - 1
        } else {
            new_index
        };
        let moved = tabs.remove(old_index);
        let insert_at = adjusted.min(tabs.len());
        tabs.insert(insert_at, moved);
        self.stack = self.stack.with_tabs(tabs, None);
        self.persist_stack();
        Outcome::changed()
    }

    pub fn set_theme(&mut self, theme: ThemePreference) -> Outcome {
        self.prefs = self.prefs.with_theme(theme);
        self.persist_prefs();
        Outcome::changed()
    }

    pub fn set_search_engine(&mut self, engine: SearchEngineId) -> Outcome {
        self.prefs = self.prefs.with_search_engine(engine);
        self.persist_prefs();
        Outcome::changed()
    }

    pub fn set_feature_flag(&mut self, key: &str, value: bool) -> Outcome {
        self.prefs = self.prefs.with_flags(self.prefs.flags.with_value(key, value));
        if key == AD_BLOCKER_KEY {
            adblock::set_enabled(value);
        }
        self.persist_prefs();
        Outcome::changed()
    }

    pub fn toggle_split_view(&mut self) -> Outcome {
        if !self.prefs.flags.enable_split_screen {
            return Outcome::nothing();
        }
        if self.split_active {
            self.split_active = false;
            self.split_tab_id = None;
            return Outcome::changed();
        }
        let other_id = self
            .stack
            .tabs
            .iter()
            .find(|tab| tab.id != self.stack.active_tab_id)
            .map(|tab| tab.id.clone());
        let id = match other_id {
            Some(id) => id,
            None => {
                let fresh = TabEntry::blank(self.next_tab_id(), self.engine().home_url);
                let fresh_id = fresh.id.clone();
                let mut tabs = self.stack.tabs.clone();
                tabs.push(fresh);
                self.stack = self.stack.with_tabs(tabs, None);
                self.persist_stack();
                fresh_id
            }
        };
        self.split_tab_id = Some(id);
        self.split_active = true;
        self.persist_stack();
        Outcome::changed()
    }

    pub fn set_platform_dark(&mut self, dark: bool) -> Outcome {
        if self.platform_dark == dark {
            return Outcome::nothing();
        }
        self.platform_dark = dark;
        Outcome::changed()
    }

    pub fn open_overlay(&mut self, overlay: u8) -> Outcome {
        if self.overlay == overlay {
            return Outcome::nothing();
        }
        self.overlay = overlay;
        Outcome::changed()
    }

    pub fn close_overlay(&mut self) -> Outcome {
        self.open_overlay(OVERLAY_NONE)
    }

    /// Menu shortcut: the update check drives the omnibox to the releases
    /// page, exactly like the old menu entry.
    pub fn check_for_updates(&mut self) -> Outcome {
        self.navigate("https://github.com/webdevshayan/phantom/releases")
    }

    /// Webview events. Only the primary slot exists in the single-webview
    /// runtime, and only real web pages are tracked.
    pub fn handle_url_event(&mut self, url: &str) -> Outcome {
        if !is_trackable(url) || self.suppress_url_events {
            return Outcome::nothing();
        }
        if url == self.stack.active().url() {
            return Outcome::nothing();
        }
        let entry = self.stack.active().navigate_to(url);
        self.replace_tab(entry);
        self.persist_stack();
        Outcome::changed()
    }

    /// A page finished loading: clear suppression and look for saved
    /// drafts on the page that saved them.
    pub fn handle_page_loaded(&mut self, _url: &str) -> Outcome {
        self.suppress_url_events = false;
        let page_url = self.stack.active().url().to_string();
        let restore_form = if page_url == BLANK_URL {
            None
        } else {
            self.store
                .load_form_snapshot(&page_url)
                .filter(|snapshot| !snapshot.is_empty())
                .map(|snapshot| snapshot.fields)
        };
        Outcome {
            changed: false,
            load: LoadRequest::None,
            restore_form,
        }
    }

    /// Streams a raw devtools instrumentation payload into the feed.
    pub fn handle_devtools_message(&mut self, raw: &str) {
        self.devtools.handle_raw(raw);
    }

    pub fn log_load_error(&mut self, message: &str) {
        self.devtools
            .append_console("error", &format!("Failed to load: {message}"));
    }

    pub fn clear_devtools(&mut self) {
        self.devtools.clear();
    }

    /// Persists a captured draft (or clears it when the collector found
    /// nothing). The URL carried by the payload wins: it is the page the
    /// user was typing on.
    pub fn save_form_draft(&mut self, page_url: &str, fields: Vec<(String, String)>) {
        if fields.is_empty() {
            let _ = self.store.clear_form_snapshot(page_url);
        } else {
            let _ = self.store.save_form_snapshot(FormSnapshot {
                page_url: page_url.to_string(),
                fields,
            });
        }
    }

    /// The snapshot the chrome renders.
    pub fn snapshot(&self) -> UiSnapshot {
        let theme = *crate::theme::Theme::resolve(self.prefs.theme, self.platform_dark);
        let active_url = self.stack.active().url();
        let tabs = self
            .stack
            .tabs
            .iter()
            .map(|tab| TabStripItem {
                id: tab.id.clone(),
                title: tab.title.clone(),
                url: tab.url().to_string(),
                active: tab.id == self.stack.active_tab_id,
            })
            .collect();
        UiSnapshot {
            revision: self.revision,
            dark: crate::theme::Theme::is_dark(self.prefs.theme, self.platform_dark),
            theme,
            omnibox: if active_url == BLANK_URL {
                String::new()
            } else {
                active_url.to_string()
            },
            can_back: self.stack.active().can_go_back(),
            can_forward: self.stack.active().can_go_forward(),
            tab_count: self.stack.tabs.len() as u32,
            active_tab_id: self.stack.active_tab_id.clone(),
            tabs,
            menu: MenuState {
                split_visible: self.prefs.flags.enable_split_screen,
                split_active: self.split_active && self.prefs.flags.enable_split_screen,
                updates_visible: self.prefs.flags.enable_auto_update,
            },
            prefs: PrefsState {
                theme_code: self.prefs.theme.code(),
                engine_code: self.prefs.search_engine.code(),
                split_screen: self.prefs.flags.enable_split_screen,
                ad_blocker: self.prefs.flags.enable_ad_blocker,
                auto_update: self.prefs.flags.enable_auto_update,
            },
            overlay: self.overlay,
        }
    }

    pub fn devtools_snapshot(&self) -> DevToolsSnapshot {
        self.devtools.snapshot()
    }

    pub fn search_engine(&self) -> &'static SearchEngine {
        self.engine()
    }

    pub fn preferences(&self) -> &Preferences {
        &self.prefs
    }

    pub fn overlay(&self) -> u8 {
        self.overlay
    }

    fn engine(&self) -> &'static SearchEngine {
        by_id(self.prefs.search_engine)
    }

    fn next_tab_id(&mut self) -> String {
        self.id_sequence += 1;
        format!("tab-{}-{}", self.space.id, self.id_sequence)
    }

    fn replace_tab(&mut self, tab: TabEntry) {
        let tabs = self
            .stack
            .tabs
            .iter()
            .map(|entry| {
                if entry.id == tab.id {
                    tab.clone()
                } else {
                    entry.clone()
                }
            })
            .collect();
        self.stack = self.stack.with_tabs(tabs, None);
    }

    fn persist_stack(&self) {
        let _ = self.store.save_tab_stack(&self.stack);
    }

    fn persist_prefs(&self) {
        let _ = self.store.save_preferences(&self.prefs);
    }

    fn bump(&mut self) {
        self.revision += 1;
    }
}

/// Real web pages only: http(s) locations track the model, everything
/// else (the bundled landing page, internal schemes) does not.
fn is_trackable(url: &str) -> bool {
    if let Some((scheme, rest)) = url.split_once("://") {
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            // The bundled app page is served from the internal host; it is
            // http on some platforms, so exclude it by host.
            let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
            let host = authority.rsplit_once('@').map(|(_, h)| h).unwrap_or(authority);
            if host.eq_ignore_ascii_case("tauri.localhost")
                || host
                    .split(':')
                    .next()
                    .unwrap_or("")
                    .eq_ignore_ascii_case("tauri.localhost")
            {
                return false;
            }
            return true;
        }
    }
    false
}

/// The form event payload from the injected collector: a JSON string of
/// `{ url, fields }`. Returns the page URL and its safe fields.
pub fn parse_form_event(raw: &str) -> Option<(String, Vec<(String, String)>)> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    let url = object.get("url")?.as_str()?.to_string();
    let fields_value = object.get("fields")?.as_object()?;
    let fields = fields_value
        .iter()
        .map(|(key, value)| {
            (
                key.clone(),
                value.as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    Some((url, fields))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omnibox::OmniboxKind;
    use crate::search::SearchEngineId;
    use crate::theme::ThemePreference;
    use crate::wire::{OVERLAY_DEVTOOLS, OVERLAY_NONE, OVERLAY_SETTINGS, OVERLAY_TABS};

    fn temp_store(tag: &str) -> SpaceStore {
        let root = std::env::temp_dir().join(format!(
            "phantom-session-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        SpaceStore::open(&root, &Space::session()).expect("store opens")
    }

    fn session(tag: &str) -> SessionState {
        let mut state = SessionState::new(Space::session(), temp_store(tag));
        state.restore();
        state
    }

    #[test]
    fn restore_persists_nothing_and_loads_the_active_tab() {
        let mut state = session("restore");
        let outcome = state.restore();
        assert!(outcome.changed);
        assert_eq!(outcome.load, LoadRequest::None, "a blank tab does not load");
        assert_eq!(state.snapshot().tab_count, 1);
    }

    #[test]
    fn restore_picks_up_saved_state() {
        // One Space directory, two sessions over it: the restart story.
        let store = temp_store("restore-saved");
        let reopen = || SpaceStore::open(store.root(), &Space::session()).expect("store opens");
        let mut previous = SessionState::new(Space::session(), reopen());
        previous.restore();
        previous.set_search_engine(SearchEngineId::Startpage);
        previous.navigate("https://one.example/");
        let mut fresh = SessionState::new(Space::session(), reopen());
        let outcome = fresh.restore();
        match outcome.load {
            LoadRequest::Url(url) => assert_eq!(url, "https://one.example/"),
            other => panic!("expected a url load, got {other:?}"),
        }
        assert_eq!(fresh.preferences().search_engine, SearchEngineId::Startpage);
        assert_eq!(fresh.snapshot().tab_count, 1);
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn navigate_resolves_and_tracks_the_model() {
        let mut state = session("navigate");
        state.restore();
        let outcome = state.navigate("example.com");
        match outcome.load {
            LoadRequest::Url(url) => assert_eq!(url, "https://example.com"),
            other => panic!("expected a url load, got {other:?}"),
        }
        let snapshot = state.snapshot();
        assert_eq!(snapshot.omnibox, "https://example.com");
        // The blank tab's location is entry zero, so the first navigation
        // already leaves one step of history behind it.
        assert!(snapshot.can_back);
        let outcome = state.navigate("how browsers work");
        match outcome.load {
            LoadRequest::Url(url) => {
                assert_eq!(url, "https://www.google.com/search?q=how%20browsers%20work")
            }
            other => panic!("expected a url load, got {other:?}"),
        }
        assert!(state.snapshot().can_back);
    }

    #[test]
    fn url_events_track_redirects_and_deduplicate() {
        let mut state = session("url-events");
        state.restore();
        state.navigate("https://one.example/");
        // The engine home dedupe: same url, no change.
        assert!(!state.handle_url_event("https://one.example/").changed);
        // A redirect target pushes onto the model.
        assert!(state.handle_url_event("https://one.example/final").changed);
        assert_eq!(state.snapshot().omnibox, "https://one.example/final");
        // Internal pages never track.
        assert!(!state.handle_url_event("about:blank").changed);
        assert!(!state
            .handle_url_event("http://tauri.localhost/index.html")
            .changed);
    }

    #[test]
    fn back_and_forward_walk_the_stack() {
        let mut state = session("walk");
        state.restore();
        state.navigate("https://one.example/");
        state.navigate("https://two.example/");
        let outcome = state.back();
        match outcome.load {
            LoadRequest::Url(url) => assert_eq!(url, "https://one.example/"),
            other => panic!("expected a url load, got {other:?}"),
        }
        assert!(state.snapshot().can_forward);
        // URL events while suppressed do not fork the stack.
        assert!(!state.handle_url_event("https://two.example/").changed);
        let outcome = state.forward();
        match outcome.load {
            LoadRequest::Url(url) => assert_eq!(url, "https://two.example/"),
            other => panic!("expected a url load, got {other:?}"),
        }
    }

    #[test]
    fn tabs_open_select_close_and_reorder() {
        let mut state = session("tabs");
        state.restore();
        state.navigate("https://one.example/");
        state.add_tab();
        assert_eq!(state.snapshot().tab_count, 2);
        let second_id = state.snapshot().active_tab_id.clone();
        assert_eq!(
            state.snapshot().omnibox,
            "https://www.google.com",
            "a fresh tab starts on the engine home"
        );

        state.select_tab("tab-session-0");
        assert_eq!(state.snapshot().active_tab_id, "tab-session-0");
        assert_eq!(state.snapshot().omnibox, "https://one.example/");

        // Reorder: move tab 0 after tab 1.
        state.reorder_tabs(0, 2);
        let snapshot = state.snapshot();
        let ids: Vec<&str> = snapshot
            .tabs
            .iter()
            .map(|tab| tab.id.as_str())
            .collect();
        assert_eq!(ids, vec![second_id.as_str(), "tab-session-0"]);

        state.close_tab(&second_id);
        assert_eq!(state.snapshot().tab_count, 1);
        assert_eq!(state.snapshot().active_tab_id, "tab-session-0");

        // Closing the last tab always leaves one fresh tab.
        state.close_tab("tab-session-0");
        assert_eq!(state.snapshot().tab_count, 1);
        assert_eq!(state.snapshot().omnibox, "https://www.google.com");
        assert!(!state.snapshot().can_back);
    }

    #[test]
    fn unknown_tab_ids_are_ignored() {
        let mut state = session("unknown");
        state.restore();
        assert!(!state.select_tab("nope").changed);
        assert!(!state.close_tab("nope").changed);
    }

    #[test]
    fn settings_persist_and_gate_the_menu() {
        let store = temp_store("settings");
        let reopen = || SpaceStore::open(store.root(), &Space::session()).expect("store opens");
        let mut state = SessionState::new(Space::session(), reopen());
        state.restore();
        assert!(!state.snapshot().menu.split_visible);
        assert!(!state.snapshot().menu.updates_visible);
        state.set_feature_flag("enable_split_screen", true);
        state.set_feature_flag("enable_auto_update", true);
        state.set_theme(ThemePreference::Dark);
        let snapshot = state.snapshot();
        assert!(snapshot.menu.split_visible);
        assert!(snapshot.menu.updates_visible);
        assert!(snapshot.dark);
        assert_eq!(snapshot.prefs.theme_code, ThemePreference::Dark.code());

        // The ad blocker flag persists with the preferences.
        state.set_feature_flag("enable_ad_blocker", true);
        assert!(state.snapshot().prefs.ad_blocker);
        state.set_feature_flag("enable_ad_blocker", false);
        assert!(!state.snapshot().prefs.ad_blocker);

        // A persisted session restores the flags.
        let mut fresh = SessionState::new(Space::session(), reopen());
        fresh.restore();
        let snapshot = fresh.snapshot();
        assert!(snapshot.menu.split_visible);
        assert!(snapshot.menu.updates_visible);
        assert_eq!(snapshot.prefs.theme_code, ThemePreference::Dark.code());
        let _ = std::fs::remove_dir_all(store.root());
    }

    #[test]
    fn split_view_requires_the_flag_and_a_partner_tab() {
        let mut state = session("split");
        state.restore();
        assert!(!state.toggle_split_view().changed, "flag off: no-op");
        state.set_feature_flag("enable_split_screen", true);
        assert!(state.toggle_split_view().changed);
        assert!(state.snapshot().menu.split_active);
        assert_eq!(
            state.snapshot().tab_count,
            2,
            "a partner tab is created on demand"
        );
        assert!(state.toggle_split_view().changed, "toggle off");
        assert!(!state.snapshot().menu.split_active);
    }

    #[test]
    fn drafts_stream_to_the_space_store() {
        let mut state = session("drafts");
        state.restore();
        state.navigate("https://example.com/login");
        state.save_form_draft(
            "https://example.com/login",
            vec![("user".to_string(), "ada".to_string())],
        );
        assert_eq!(
            state
                .store
                .load_form_snapshot("https://example.com/login")
                .expect("draft saved")
                .fields
                .len(),
            1
        );

        // A page load restores the draft for the page that saved it.
        let outcome = state.handle_page_loaded("https://example.com/login");
        let fields = outcome.restore_form.expect("draft restored");
        assert_eq!(fields, vec![("user".to_string(), "ada".to_string())]);

        // Empty captures clear the draft.
        state.save_form_draft("https://example.com/login", Vec::new());
        let outcome = state.handle_page_loaded("https://example.com/login");
        assert_eq!(outcome.restore_form, None);
    }

    #[test]
    fn overlays_open_and_close() {
        let mut state = session("overlays");
        state.restore();
        assert_eq!(state.snapshot().overlay, OVERLAY_NONE);
        state.open_overlay(OVERLAY_TABS);
        assert_eq!(state.snapshot().overlay, OVERLAY_TABS);
        state.open_overlay(OVERLAY_SETTINGS);
        assert_eq!(state.snapshot().overlay, OVERLAY_SETTINGS);
        state.open_overlay(OVERLAY_DEVTOOLS);
        assert_eq!(state.snapshot().overlay, OVERLAY_DEVTOOLS);
        state.close_overlay();
        assert_eq!(state.snapshot().overlay, OVERLAY_NONE);
    }

    #[test]
    fn devtools_feed_flows_through_the_session() {
        let mut state = session("devtools");
        state.restore();
        state.handle_devtools_message(
            r#"{"kind":"console","level":"warn","message":"slow","at":1700000000000}"#,
        );
        state.log_load_error("net::ERR_CACHE_MISS");
        let feed = state.devtools_snapshot();
        assert_eq!(feed.console.len(), 2);
        assert_eq!(feed.console[1].message, "Failed to load: net::ERR_CACHE_MISS");
        state.clear_devtools();
        assert_eq!(state.devtools_snapshot().console.len(), 0);
    }

    #[test]
    fn form_events_parse_from_the_collector_payload() {
        let raw = r#"{"url":"https://example.com/form","fields":{"user":"ada","skip":""}"#;
        let (url, fields) = parse_form_event(raw).expect("parses");
        assert_eq!(url, "https://example.com/form");
        assert_eq!(fields.len(), 2);
        assert!(parse_form_event("garbage").is_none());
        assert!(parse_form_event(r#"{"url":"https://x"}"#).is_none());
    }

    #[test]
    fn search_engine_exposes_the_catalog_choice() {
        let mut state = session("engine");
        state.restore();
        assert_eq!(state.search_engine().id, SearchEngineId::Google);
        state.set_search_engine(SearchEngineId::Bing);
        assert_eq!(state.search_engine().id, SearchEngineId::Bing);
        let outcome = state.navigate("hotels near me");
        match outcome.load {
            LoadRequest::Url(url) => {
                assert!(url.starts_with("https://www.bing.com/search?q="))
            }
            other => panic!("expected a url load, got {other:?}"),
        }
    }

    #[test]
    fn omnibox_search_targets_reach_the_model() {
        let mut state = session("kinds");
        state.restore();
        let url_target = resolve_omnibox_input("example.com", state.search_engine());
        assert_eq!(url_target.kind, OmniboxKind::Url);
        let search_target = resolve_omnibox_input("example dot com", state.search_engine());
        assert_eq!(search_target.kind, OmniboxKind::Search);
    }
}
