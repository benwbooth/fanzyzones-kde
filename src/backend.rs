use anyhow::{Context, Result};
use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Default)]
pub struct FanzyBackendRust {
    settings_json: QString,
    status: QString,
    tray_command_json: QString,
    tray_icon_source: QString,
}

#[derive(Default)]
pub struct EditorBridgeRust {
    input_json: QString,
}

// Channels between the Rust side that launches the editor and the in-process QML
// editor. The editor runs on the main thread in its own process, so plain
// statics suffice: Rust stuffs the input before exec(), QML hands back the
// result via submit().
static EDITOR_INPUT: Mutex<Option<String>> = Mutex::new(None);
static EDITOR_RESULT: Mutex<Option<String>> = Mutex::new(None);

pub fn set_editor_input(json: String) {
    if let Ok(mut guard) = EDITOR_INPUT.lock() {
        *guard = Some(json);
    }
}

pub fn take_editor_result() -> Option<String> {
    EDITOR_RESULT.lock().ok().and_then(|mut guard| guard.take())
}

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("QtQml/qqmlregistration.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, settings_json)]
        #[qproperty(QString, status)]
        #[qproperty(QString, tray_command_json)]
        #[qproperty(QString, tray_icon_source)]
        type FanzyBackend = super::FanzyBackendRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut FanzyBackend>);

        #[qinvokable]
        fn invoke_action(self: Pin<&mut FanzyBackend>, payload: &QString) -> bool;

        #[qobject]
        #[qml_element]
        #[qproperty(QString, input_json)]
        type EditorBridge = super::EditorBridgeRust;

        // The QML editor calls this on Save with the layout as JSON.
        #[qinvokable]
        fn submit(self: Pin<&mut EditorBridge>, result: &QString);
    }

    impl cxx_qt::Initialize for FanzyBackend {}
    impl cxx_qt::Threading for FanzyBackend {}
    impl cxx_qt::Initialize for EditorBridge {}
}

impl cxx_qt::Initialize for ffi::EditorBridge {
    fn initialize(mut self: Pin<&mut Self>) {
        let input = EDITOR_INPUT
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_default();
        self.as_mut().set_input_json(QString::from(input));
    }
}

impl ffi::EditorBridge {
    fn submit(self: Pin<&mut Self>, result: &QString) {
        if let Ok(mut guard) = EDITOR_RESULT.lock() {
            *guard = Some(String::from(result));
        }
    }
}

impl cxx_qt::Initialize for ffi::FanzyBackend {
    fn initialize(mut self: Pin<&mut Self>) {
        tracing::info!("initializing FanzyZones QML backend");
        let qt_thread = self.as_ref().get_ref().qt_thread();
        self.as_mut().refresh();
        start_tray_server(qt_thread);
    }
}

impl ffi::FanzyBackend {
    fn refresh(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_tray_icon_source(QString::from(crate::tray_icon_source_url()));
        match initialize_state() {
            Ok((settings_json, status)) => {
                self.as_mut()
                    .set_settings_json(QString::from(settings_json));
                self.as_mut().set_status(QString::from(status));
            }
            Err(err) => {
                self.as_mut()
                    .set_status(QString::from(format!("Error: {err:#}")));
            }
        }
    }

    fn invoke_action(mut self: Pin<&mut Self>, payload: &QString) -> bool {
        let payload = String::from(payload);
        match handle_action_payload(&payload) {
            Ok(ActionResult {
                settings_json,
                status,
            }) => {
                self.as_mut()
                    .set_settings_json(QString::from(settings_json));
                self.as_mut().set_status(QString::from(status));
                true
            }
            Err(err) => {
                self.as_mut()
                    .set_status(QString::from(format!("Error: {err:#}")));
                false
            }
        }
    }
}

struct ActionResult {
    settings_json: String,
    status: String,
}

fn initialize_state() -> Result<(String, String)> {
    let controller = crate::KwinController::from_environment()?;
    let mut settings = crate::load_and_save_settings()?;
    let startup_status = format!(
        "Setting up KWin integration from {}...",
        controller.script_dir().display()
    );
    let settings_json = settings.compact_json()?;
    let status = match block_on(controller.sync(&settings, true)) {
        Ok(()) => {
            settings = crate::load_and_save_settings()?;
            "KWin integration ready".to_string()
        }
        Err(err) => format!("Error: {err:#}"),
    };
    let status = if status.is_empty() {
        startup_status
    } else {
        status
    };
    Ok((settings.compact_json().unwrap_or(settings_json), status))
}

fn handle_action_payload(payload: &str) -> Result<ActionResult> {
    if payload.contains("\"action\":\"debugPlacement\"") {
        crate::log_placement_debug(payload);
        return current_state("KWin integration ready");
    }

    let request = crate::parse_visual_menu_payload(payload)?;
    let _close_menu = request.close_menu;
    let mut settings = crate::load_and_save_settings()?;
    let controller = crate::KwinController::from_environment()?;
    let should_quit = block_on(crate::handle_visual_menu_action(
        request.action,
        &controller,
        &mut settings,
    ))?;
    let status = if should_quit {
        "Quitting FanzyZones".to_string()
    } else {
        "KWin integration ready".to_string()
    };
    let settings = crate::load_and_save_settings().unwrap_or(settings);
    Ok(ActionResult {
        settings_json: settings.compact_json()?,
        status,
    })
}

fn current_state(status: &str) -> Result<ActionResult> {
    let settings = crate::load_and_save_settings()?;
    Ok(ActionResult {
        settings_json: settings.compact_json()?,
        status: status.into(),
    })
}

fn block_on<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create Tokio runtime")?
        .block_on(future)
}

static TRAY_SERVER_STARTED: AtomicBool = AtomicBool::new(false);

struct StatusNotifierItem {
    qt_thread: CxxQtThread<ffi::FanzyBackend>,
    sequence: AtomicU64,
}

impl StatusNotifierItem {
    fn new(qt_thread: CxxQtThread<ffi::FanzyBackend>) -> Self {
        Self {
            qt_thread,
            sequence: AtomicU64::new(0),
        }
    }

    fn emit_tray_command(&self, reason: &str, x: i32, y: i32) {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let valid_anchor = x != 0 || y != 0;
        let source = if valid_anchor { "trayClick" } else { "unknown" };
        let command = TrayCommand {
            action: "toggleTrayMenu",
            reason,
            sequence,
            anchor: TrayAnchor {
                valid: valid_anchor,
                x,
                y,
                width: 0,
                height: 0,
                source,
            },
        };
        let command_json = match serde_json::to_string(&command) {
            Ok(command_json) => command_json,
            Err(err) => {
                tracing::warn!(error = ?err, "failed to encode tray command");
                return;
            }
        };

        if std::env::var_os("FANZYZONES_KDE_DEBUG_TRAY").is_some() {
            eprintln!("FanzyZones tray {reason} x={x} y={y} source={source}");
        }

        let _ = self.qt_thread.queue(move |mut backend| {
            backend
                .as_mut()
                .set_tray_command_json(QString::from(command_json));
        });
    }
}

#[derive(Serialize)]
struct TrayCommand<'a> {
    action: &'a str,
    reason: &'a str,
    sequence: u64,
    anchor: TrayAnchor<'a>,
}

#[derive(Serialize)]
struct TrayAnchor<'a> {
    valid: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    source: &'a str,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl StatusNotifierItem {
    fn context_menu(&self, x: i32, y: i32) -> zbus::fdo::Result<()> {
        self.emit_tray_command("ContextMenu", x, y);
        Ok(())
    }

    fn activate(&self, x: i32, y: i32) -> zbus::fdo::Result<()> {
        self.emit_tray_command("Activate", x, y);
        Ok(())
    }

    fn secondary_activate(&self, x: i32, y: i32) -> zbus::fdo::Result<()> {
        self.emit_tray_command("SecondaryActivate", x, y);
        Ok(())
    }

    fn scroll(&self, _delta: i32, _orientation: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    #[zbus(property)]
    fn category(&self) -> zbus::fdo::Result<&str> {
        Ok("ApplicationStatus")
    }

    #[zbus(property)]
    fn id(&self) -> zbus::fdo::Result<&str> {
        Ok("fanzyzones-kde")
    }

    #[zbus(property)]
    fn title(&self) -> zbus::fdo::Result<&str> {
        Ok("FanzyZones KDE")
    }

    #[zbus(property)]
    fn status(&self) -> zbus::fdo::Result<&str> {
        Ok("Active")
    }

    #[zbus(property)]
    fn window_id(&self) -> zbus::fdo::Result<i32> {
        Ok(0)
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> zbus::fdo::Result<String> {
        Ok(crate::tray_icon_theme_path())
    }

    #[zbus(property)]
    fn menu(&self) -> zbus::fdo::Result<zbus::zvariant::ObjectPath<'static>> {
        Ok(zbus::zvariant::ObjectPath::from_static_str_unchecked(
            "/NO_DBUSMENU",
        ))
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> zbus::fdo::Result<bool> {
        Ok(false)
    }

    #[zbus(property)]
    fn icon_name(&self) -> zbus::fdo::Result<&str> {
        Ok("fanzyzones-kde")
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> zbus::fdo::Result<Vec<(i32, i32, Vec<u8>)>> {
        Ok(Vec::new())
    }

    #[zbus(property)]
    fn overlay_icon_name(&self) -> zbus::fdo::Result<&str> {
        Ok("")
    }

    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> zbus::fdo::Result<Vec<(i32, i32, Vec<u8>)>> {
        Ok(Vec::new())
    }

    #[zbus(property)]
    fn attention_icon_name(&self) -> zbus::fdo::Result<&str> {
        Ok("")
    }

    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> zbus::fdo::Result<Vec<(i32, i32, Vec<u8>)>> {
        Ok(Vec::new())
    }

    #[zbus(property)]
    fn attention_movie_name(&self) -> zbus::fdo::Result<&str> {
        Ok("")
    }
}

fn start_tray_server(qt_thread: CxxQtThread<ffi::FanzyBackend>) {
    if TRAY_SERVER_STARTED.swap(true, Ordering::SeqCst) {
        tracing::debug!("FanzyZones tray status notifier already started");
        return;
    }

    tracing::info!("starting FanzyZones tray status notifier thread");
    let spawn_result = std::thread::Builder::new()
        .name("fanzyzones-kde-status-notifier".into())
        .spawn(move || {
            if let Err(err) = run_tray_server_blocking(qt_thread) {
                TRAY_SERVER_STARTED.store(false, Ordering::SeqCst);
                tracing::warn!(error = ?err, "FanzyZones tray status notifier failed");
            }
        });

    if let Err(err) = spawn_result {
        TRAY_SERVER_STARTED.store(false, Ordering::SeqCst);
        tracing::warn!(error = ?err, "failed to start FanzyZones tray status notifier thread");
    }
}

fn run_tray_server_blocking(qt_thread: CxxQtThread<ffi::FanzyBackend>) -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create tray status notifier Tokio runtime")?
        .block_on(run_tray_server(qt_thread))
}

async fn run_tray_server(qt_thread: CxxQtThread<ffi::FanzyBackend>) -> Result<()> {
    tracing::info!("connecting FanzyZones StatusNotifierItem to session bus");
    let item = StatusNotifierItem::new(qt_thread);
    let connection = zbus::connection::Builder::session()
        .context("create StatusNotifierItem session bus builder")?
        .serve_at("/StatusNotifierItem", item)
        .context("serve StatusNotifierItem object")?
        .build()
        .await
        .context("connect StatusNotifierItem to session bus")?;
    let service_name = connection
        .unique_name()
        .context("read StatusNotifierItem unique bus name")?
        .as_str()
        .to_string();
    tracing::info!(service_name, "registering FanzyZones StatusNotifierItem");

    connection
        .call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.kde.StatusNotifierWatcher"),
            "RegisterStatusNotifierItem",
            &(service_name.as_str()),
        )
        .await
        .context("register FanzyZones StatusNotifierItem")?;
    tracing::info!("FanzyZones StatusNotifierItem registered");

    std::future::pending::<()>().await;
    #[allow(unreachable_code)]
    Ok(())
}
