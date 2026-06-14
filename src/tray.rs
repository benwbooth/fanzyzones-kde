use crate::config::Settings;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct FanzyTray {
    pub settings: Settings,
    pub status: String,
    pub icon_theme_path: String,
    pub sender: UnboundedSender<TrayMessage>,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    StartupSync,
    OpenVisualMenu {
        source: &'static str,
        x: i32,
        y: i32,
        status: String,
    },
}

impl ksni::Tray for FanzyTray {
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        "fanzyzones-kde".into()
    }

    fn title(&self) -> String {
        "FanzyZones KDE".into()
    }

    fn icon_theme_path(&self) -> String {
        self.icon_theme_path.clone()
    }

    fn icon_name(&self) -> String {
        "fanzyzones-kde".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.sender.send(TrayMessage::OpenVisualMenu {
            source: "activate",
            x: _x,
            y: _y,
            status: self.status.clone(),
        });
    }

    fn context_menu(&mut self, x: i32, y: i32) {
        let _ = self.sender.send(TrayMessage::OpenVisualMenu {
            source: "context_menu",
            x,
            y,
            status: self.status.clone(),
        });
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "FanzyZones KDE".into(),
            description: format!(
                "Active layout: {}\n{}",
                self.settings.active_layout_name(),
                self.status
            ),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        Vec::new()
    }
}
