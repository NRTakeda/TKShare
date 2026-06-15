use gettextrs::gettext;

use crate::config::APP_ID;

#[derive(Debug)]
pub struct Tray {
    pub tx: tokio::sync::mpsc::Sender<TrayMessage>,
    /// Whether the device is currently visible to others. Used to reflect
    /// the current state in the "Visible to others" checkmark item.
    pub is_visible: bool,
}

#[derive(Debug, Clone)]
pub enum TrayMessage {
    OpenWindow,
    SendFiles,
    ToggleVisibility,
    OpenReceivedFiles,
    OpenPreferences,
    Quit,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        APP_ID.into()
    }
    fn icon_name(&self) -> String {
        // Use the per-profile APP_ID so the right symbolic icon is found in
        // both release (io.github.NRTakeda.TKShare) and devel
        // (io.github.NRTakeda.TKShare.Devel) builds.
        format!("{APP_ID}-symbolic")
    }
    fn title(&self) -> String {
        gettext("TKShare")
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: gettext("Open"),
                activate: Box::new(move |this: &mut Self| {
                    _ = this.tx.try_send(TrayMessage::OpenWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Send Files…"),
                icon_name: "list-add-symbolic".into(),
                activate: Box::new(move |this: &mut Self| {
                    _ = this.tx.try_send(TrayMessage::SendFiles);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: gettext("Visible to Others"),
                checked: self.is_visible,
                activate: Box::new(move |this: &mut Self| {
                    _ = this.tx.try_send(TrayMessage::ToggleVisibility);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Open Received Files"),
                icon_name: "folder-symbolic".into(),
                activate: Box::new(move |this: &mut Self| {
                    _ = this.tx.try_send(TrayMessage::OpenReceivedFiles);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: gettext("Preferences"),
                icon_name: "preferences-system-symbolic".into(),
                activate: Box::new(move |this: &mut Self| {
                    _ = this.tx.try_send(TrayMessage::OpenPreferences);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: gettext("Exit"),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(move |this: &mut Self| {
                    _ = this.tx.try_send(TrayMessage::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
