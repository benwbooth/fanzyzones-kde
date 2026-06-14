use crate::config::Settings;
use ksni::menu::{RadioGroup, RadioItem, StandardItem, SubMenu};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub struct FanzyTray {
    pub settings: Settings,
    pub status: String,
    pub sender: UnboundedSender<TrayMessage>,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    Sync,
    ReloadKwin,
    OpenSettings,
    ReloadSettings,
    SetLayout(usize),
    SnapZone(usize),
    NextZone,
    PreviousZone,
    Quit,
}

impl ksni::Tray for FanzyTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "fanzyzones-kde".into()
    }

    fn title(&self) -> String {
        "FanzyZones KDE".into()
    }

    fn icon_name(&self) -> String {
        "preferences-system-windows".into()
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
        use ksni::menu::MenuItem;

        let mut items: Vec<MenuItem<Self>> = vec![StandardItem {
            label: format!("Active: {}", self.settings.active_layout_name()),
            enabled: false,
            icon_name: "view-grid".into(),
            ..Default::default()
        }
        .into()];

        items.push(
            SubMenu {
                label: "Layouts".into(),
                icon_name: "view-list-icons".into(),
                submenu: vec![RadioGroup {
                    selected: self.settings.active_layout,
                    select: Box::new(|this: &mut Self, selected| {
                        let _ = this.sender.send(TrayMessage::SetLayout(selected));
                    }),
                    options: self
                        .settings
                        .layouts
                        .iter()
                        .map(|layout| RadioItem {
                            label: layout.name.clone(),
                            ..Default::default()
                        })
                        .collect(),
                }
                .into()],
                ..Default::default()
            }
            .into(),
        );

        let active_layout = self
            .settings
            .layouts
            .get(self.settings.active_layout)
            .cloned();
        if let Some(layout) = active_layout {
            items.push(
                SubMenu {
                    label: "Snap Focused Window".into(),
                    icon_name: "transform-move".into(),
                    submenu: layout
                        .zones
                        .iter()
                        .enumerate()
                        .map(|(index, zone)| {
                            StandardItem {
                                label: format!("{} {}", index + 1, zone.name),
                                icon_name: "snap-orthogonal".into(),
                                activate: Box::new(move |this: &mut Self| {
                                    let _ = this.sender.send(TrayMessage::SnapZone(index));
                                }),
                                ..Default::default()
                            }
                            .into()
                        })
                        .collect(),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.extend([
            MenuItem::Separator,
            StandardItem {
                label: "Previous Zone".into(),
                icon_name: "go-previous".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::PreviousZone);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next Zone".into(),
                icon_name: "go-next".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::NextZone);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Install or Upgrade KWin Script".into(),
                icon_name: "run-install".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::Sync);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Reload KWin".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::ReloadKwin);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Reload Settings".into(),
                icon_name: "document-revert".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::ReloadSettings);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Settings JSON".into(),
                icon_name: "document-edit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: self.status.clone(),
                enabled: false,
                icon_name: "dialog-information".into(),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayMessage::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]);

        items
    }
}
