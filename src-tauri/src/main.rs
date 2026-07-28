// Minimal Browser Lite — Tauri edition.
//
// Uses the OS-native webview (WebView2 on Windows, WKWebView on macOS,
// WebKitGTK on Linux) instead of bundling Chromium. Result: a few MB
// installer and ~20-40MB idle RAM vs. Electron's 100MB+ baseline.
//
// TRADE-OFF: no Chrome extension support. WKWebView/WebKitGTK don't
// implement Chromium's extension APIs at all; WebView2 (Windows) is
// Chromium-based but the fork here targets all three OSes uniformly,
// so extensions are left out entirely rather than being Windows-only.
//
// NOTE: This file targets the Tauri 2.x multiwebview APIs from memory.
// No Rust toolchain was available to compile/verify this in the
// generating environment — install Rust + `cargo tauri dev` and expect
// to reconcile small API differences against your exact tauri version.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow};

const TOOLBAR_HEIGHT: f64 = 80.0;

#[derive(Serialize, Deserialize, Clone)]
struct HistoryEntry {
    url: String,
    title: String,
    time: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Bookmark {
    url: String,
    title: String,
}

struct TabState {
    order: Vec<String>, // webview labels, in tab order
    titles: HashMap<String, String>,
    urls: HashMap<String, String>,
    active: Option<String>,
    next_id: u64,
}

struct AppState {
    tabs: Mutex<TabState>,
    history: Mutex<Vec<HistoryEntry>>,
    bookmarks: Mutex<Vec<Bookmark>>,
    data_dir: Mutex<PathBuf>,
}

fn history_file(dir: &PathBuf) -> PathBuf {
    dir.join("history.json")
}
fn bookmarks_file(dir: &PathBuf) -> PathBuf {
    dir.join("bookmarks.json")
}

fn read_json<T: serde::de::DeserializeOwned + Default>(path: &PathBuf) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn write_json<T: Serialize>(path: &PathBuf, data: &T) {
    if let Ok(s) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, s);
    }
}

fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains(' ') || !trimmed.contains('.') {
        format!(
            "https://www.google.com/search?q={}",
            urlencode(trimmed)
        )
    } else {
        format!("https://{}", trimmed)
    }
}

fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

fn layout_tabs(window: &WebviewWindow, state: &TabState) {
    let size = window.inner_size().unwrap_or(tauri::PhysicalSize::new(1100, 720));
    let scale = window.scale_factor().unwrap_or(1.0);
    let logical_w = size.width as f64 / scale;
    let logical_h = size.height as f64 / scale;

    for label in &state.order {
        if let Some(wv) = window.get_webview(label) {
            if Some(label.clone()) == state.active {
                let _ = wv.set_position(LogicalPosition::new(0.0, TOOLBAR_HEIGHT));
                let _ = wv.set_size(LogicalSize::new(logical_w, logical_h - TOOLBAR_HEIGHT));
                let _ = wv.show();
            } else {
                // park inactive webviews off-screen with zero size instead of
                // destroying them, so back/forward/scroll state is kept warm
                // for cheap re-activation (still costs some idle memory —
                // for true suspension, close_tab + recreate on demand instead).
                let _ = wv.set_size(LogicalSize::new(0.0, 0.0));
                let _ = wv.hide();
            }
        }
    }
}

fn emit_tabs_updated(window: &WebviewWindow, state: &TabState) {
    let tabs: Vec<_> = state
        .order
        .iter()
        .map(|label| {
            serde_json::json!({
                "id": label,
                "title": state.titles.get(label).cloned().unwrap_or_else(|| "New Tab".into()),
                "url": state.urls.get(label).cloned().unwrap_or_default(),
            })
        })
        .collect();
    let _ = window.emit(
        "tabs-updated",
        serde_json::json!({ "tabs": tabs, "activeId": state.active }),
    );
}

#[tauri::command]
fn new_tab(app: tauri::AppHandle, window: WebviewWindow, url: Option<String>) -> String {
    let state = app.state::<AppState>();
    let mut tabs = state.tabs.lock().unwrap();
    tabs.next_id += 1;
    let label = format!("tab-{}", tabs.next_id);
    let target = normalize_url(&url.unwrap_or_else(|| "https://www.google.com".into()));

    let nav_app = app.clone();
    let nav_label = label.clone();
    let builder = tauri::webview::WebviewBuilder::new(&label, WebviewUrl::External(target.parse().unwrap()))
        .initialization_script(
            r#"
            new MutationObserver(() => {
              window.__TAURI__.core.invoke('set_tab_title', { id: window.__MB_LABEL__, title: document.title });
            }).observe(document.querySelector('title') || document.documentElement, { subtree: true, characterData: true, childList: true });
            "#,
        )
        .on_navigation(move |url| {
            let state = nav_app.state::<AppState>();
            let mut tabs = state.tabs.lock().unwrap();
            tabs.urls.insert(nav_label.clone(), url.to_string());
            drop(tabs);
            push_history(&nav_app, url.to_string(), url.to_string());
            true
        });
    let webview = window.add_child(
        builder,
        LogicalPosition::new(0.0, TOOLBAR_HEIGHT),
        LogicalSize::new(1100.0, 720.0 - TOOLBAR_HEIGHT),
    ).expect("failed to create tab webview");
    // expose the tab's own label to its injected script (window.__MB_LABEL__)
    let _ = webview.eval(&format!("window.__MB_LABEL__ = '{}';", label));

    tabs.order.push(label.clone());
    tabs.urls.insert(label.clone(), target);
    tabs.titles.insert(label.clone(), "New Tab".into());
    tabs.active = Some(label.clone());
    layout_tabs(&window, &tabs);
    emit_tabs_updated(&window, &tabs);
    label
}

#[tauri::command]
fn switch_tab(app: tauri::AppHandle, window: WebviewWindow, id: String) {
    let state = app.state::<AppState>();
    let mut tabs = state.tabs.lock().unwrap();
    if tabs.order.contains(&id) {
        tabs.active = Some(id);
        layout_tabs(&window, &tabs);
        emit_tabs_updated(&window, &tabs);
    }
}

#[tauri::command]
fn close_tab(app: tauri::AppHandle, window: WebviewWindow, id: String) {
    let state = app.state::<AppState>();
    let mut tabs = state.tabs.lock().unwrap();
    if let Some(pos) = tabs.order.iter().position(|l| l == &id) {
        if let Some(wv) = window.get_webview(&id) {
            let _ = wv.close();
        }
        tabs.order.remove(pos);
        tabs.titles.remove(&id);
        tabs.urls.remove(&id);
        if tabs.active.as_deref() == Some(id.as_str()) {
            let fallback = tabs.order.get(pos.saturating_sub(1)).or(tabs.order.first()).cloned();
            tabs.active = fallback;
        }
        if tabs.order.is_empty() {
            drop(tabs);
            new_tab(app, window, None);
            return;
        }
        layout_tabs(&window, &tabs);
        emit_tabs_updated(&window, &tabs);
    }
}

#[tauri::command]
fn navigate(app: tauri::AppHandle, window: WebviewWindow, url: String) {
    let state = app.state::<AppState>();
    let tabs = state.tabs.lock().unwrap();
    if let Some(active) = tabs.active.clone() {
        if let Some(wv) = window.get_webview(&active) {
            let target = normalize_url(&url);
            let _ = wv.navigate(target.parse().unwrap());
        }
    }
}

#[tauri::command]
fn go_back(app: tauri::AppHandle, window: WebviewWindow) {
    with_active_webview(&app, &window, |wv| { let _ = wv.eval("history.back()"); });
}
#[tauri::command]
fn go_forward(app: tauri::AppHandle, window: WebviewWindow) {
    with_active_webview(&app, &window, |wv| { let _ = wv.eval("history.forward()"); });
}
#[tauri::command]
fn reload(app: tauri::AppHandle, window: WebviewWindow) {
    with_active_webview(&app, &window, |wv| { let _ = wv.eval("location.reload()"); });
}

fn with_active_webview<F: FnOnce(&tauri::Webview)>(app: &tauri::AppHandle, window: &WebviewWindow, f: F) {
    let state = app.state::<AppState>();
    let tabs = state.tabs.lock().unwrap();
    if let Some(active) = tabs.active.clone() {
        if let Some(wv) = window.get_webview(&active) {
            f(&wv);
        }
    }
}

fn push_history(app: &tauri::AppHandle, url: String, title: String) {
    if url.starts_with("about:") { return; }
    let state = app.state::<AppState>();
    let mut history = state.history.lock().unwrap();
    if history.last().map(|h| h.url == url).unwrap_or(false) { return; }
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    history.push(HistoryEntry { url, title, time });
    if history.len() > 2000 { history.remove(0); }
    write_json(&history_file(&state.data_dir.lock().unwrap()), &*history);
}

#[tauri::command]
fn set_tab_title(app: tauri::AppHandle, window: WebviewWindow, id: String, title: String) {
    let state = app.state::<AppState>();
    let mut tabs = state.tabs.lock().unwrap();
    tabs.titles.insert(id, title);
    emit_tabs_updated(&window, &tabs);
}

#[tauri::command]
fn get_history(app: tauri::AppHandle) -> Vec<HistoryEntry> {
    app.state::<AppState>().history.lock().unwrap().clone()
}
#[tauri::command]
fn clear_history(app: tauri::AppHandle) {
    let state = app.state::<AppState>();
    let mut history = state.history.lock().unwrap();
    history.clear();
    write_json(&history_file(&state.data_dir.lock().unwrap()), &*history);
}

#[tauri::command]
fn get_bookmarks(app: tauri::AppHandle) -> Vec<Bookmark> {
    app.state::<AppState>().bookmarks.lock().unwrap().clone()
}
#[tauri::command]
fn toggle_bookmark(app: tauri::AppHandle, url: String, title: String) {
    let state = app.state::<AppState>();
    let mut bookmarks = state.bookmarks.lock().unwrap();
    if let Some(pos) = bookmarks.iter().position(|b| b.url == url) {
        bookmarks.remove(pos);
    } else {
        bookmarks.push(Bookmark { url, title });
    }
    write_json(&bookmarks_file(&state.data_dir.lock().unwrap()), &*bookmarks);
}
#[tauri::command]
fn remove_bookmark(app: tauri::AppHandle, url: String) {
    let state = app.state::<AppState>();
    let mut bookmarks = state.bookmarks.lock().unwrap();
    bookmarks.retain(|b| b.url != url);
    write_json(&bookmarks_file(&state.data_dir.lock().unwrap()), &*bookmarks);
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("app data dir");
            fs::create_dir_all(&data_dir).ok();
            let history: Vec<HistoryEntry> = read_json(&history_file(&data_dir));
            let bookmarks: Vec<Bookmark> = read_json(&bookmarks_file(&data_dir));

            app.manage(AppState {
                tabs: Mutex::new(TabState {
                    order: vec![],
                    titles: HashMap::new(),
                    urls: HashMap::new(),
                    active: None,
                    next_id: 0,
                }),
                history: Mutex::new(history),
                bookmarks: Mutex::new(bookmarks),
                data_dir: Mutex::new(data_dir),
            });

            let window = app.get_webview_window("main").unwrap();
            new_tab(app.handle().clone(), window, None);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            new_tab,
            switch_tab,
            close_tab,
            navigate,
            go_back,
            go_forward,
            reload,
            get_history,
            clear_history,
            get_bookmarks,
            toggle_bookmark,
            remove_bookmark,
            set_tab_title
        ])
        .run(tauri::generate_context!())
        .expect("error while running Minimal Browser Lite");
}
