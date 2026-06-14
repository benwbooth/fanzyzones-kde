use anyhow::{Context, Result};
use cxx_qt_lib::QString;
use std::future::Future;
use std::pin::Pin;

#[derive(Default)]
pub struct FanzyBackendRust {
    settings_json: QString,
    status: QString,
    tray_icon_source: QString,
}

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, settings_json)]
        #[qproperty(QString, status)]
        #[qproperty(QString, tray_icon_source)]
        type FanzyBackend = super::FanzyBackendRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut FanzyBackend>);

        #[qinvokable]
        fn invoke_action(self: Pin<&mut FanzyBackend>, payload: &QString) -> bool;
    }

    impl cxx_qt::Initialize for FanzyBackend {}
}

impl cxx_qt::Initialize for ffi::FanzyBackend {
    fn initialize(self: Pin<&mut Self>) {
        self.refresh();
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
