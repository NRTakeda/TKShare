use crate::{
    ext::MessageExt,
    objects::{self, TransferState, send_transfer::SendRequestState},
    tokio_runtime, widgets,
    window::PacketApplicationWindow,
};

use adw::prelude::*;
use adw::subclass::prelude::*;
use formatx::formatx;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib, glib::clone};
use rqs_lib::channel::{ChannelMessage, MessageClient};

fn get_model_item_from_listbox_row<T>(
    model: &gio::ListStore,
    list_box: &gtk::ListBox,
    row: &gtk::ListBoxRow,
) -> Option<T>
where
    T: IsA<glib::Object>,
{
    let mut pos = 0;
    while let Some(x) = list_box.row_at_index(pos) {
        if x == *row {
            break;
        }
        pos = pos + 1;
    }

    model
        .item(pos as u32)
        .and_then(|it| it.downcast::<T>().ok())
}

/// Don't try to reuse a ListBoxRow...\
/// ListBoxRow can be attached to a different model's widget
fn get_listbox_row_from_model_item<T>(
    model: &gio::ListStore,
    list_box: &gtk::ListBox,
    model_item: &T,
) -> Option<gtk::ListBoxRow>
where
    T: IsA<glib::Object>,
{
    let mut pos = 0;
    while let Some(x) = model.item(pos) {
        if x.downcast_ref::<T>()? == model_item {
            break;
        }
        pos = pos + 1;
    }

    list_box.row_at_index(pos as i32)
}

pub fn handle_recipient_card_clicked(
    win: &PacketApplicationWindow,
    list_box: &gtk::ListBox,
    row: &gtk::ListBoxRow,
) {
    let imp = win.imp();

    let model_item =
        get_model_item_from_listbox_row::<SendRequestState>(&imp.recipient_model, list_box, row)
            .expect("Index should be valid since model and ListBox are related");

    emit_send_files(win, &model_item);

    // Only reset this on Cancelled
    row.set_activatable(false);
}

fn emit_send_files(win: &PacketApplicationWindow, model_item: &SendRequestState) {
    let imp = win.imp();

    let endpoint_info = model_item.endpoint_info();
    let files_to_send = model_item.imp().files.borrow().clone();

    // Only one transfer at a time is supported by the protocol
    // Whether it be receiving or sending
    let will_be_queued = imp
        .recipient_model
        .iter::<SendRequestState>()
        .filter_map(|it| it.ok())
        .find(|it| match it.transfer_state() {
            TransferState::RequestedForConsent | TransferState::OngoingTransfer => true,
            _ => false,
        })
        .is_some();
    if will_be_queued {
        model_item.set_transfer_state(TransferState::Queued);
    }

    tokio_runtime().spawn(clone!(
        #[weak(rename_to = file_sender)]
        imp.file_sender,
        // #[weak]
        // model_item,
        async move {
            // FIXME: Set Failed state on Err and update UI on Failed state change
            // model_item.set_transfer_state(TransferState::Failed);
            file_sender
                .lock()
                .await
                .as_mut()
                .expect("RQS .file_sender must be set")
                .send(rqs_lib::SendInfo {
                    id: endpoint_info.id.clone(),
                    name: endpoint_info
                        .name
                        .clone()
                        .unwrap_or(gettext("Unknown device")),
                    addr: format!(
                        "{}:{}",
                        endpoint_info.ip.clone().unwrap_or_default(),
                        endpoint_info.port.clone().unwrap_or_default()
                    ),
                    ob: rqs_lib::OutboundPayload::Files(files_to_send),
                })
                .await
                .unwrap();
        }
    ));
}

pub fn create_recipient_card(
    win: &PacketApplicationWindow,
    _model: &gio::ListStore,
    model_item: &SendRequestState,
    init_model_state: Option<()>,
) -> adw::Bin {
    let imp = win.imp();

    if init_model_state.is_some() {
        model_item.set_device_name(model_item.endpoint_info().name.clone().unwrap_or_default());

        let files_to_send = imp
            .manage_files_model
            .iter::<gio::File>()
            .filter_map(|it| it.ok())
            .filter_map(|it| it.path())
            .map(|it| it.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        *model_item.imp().files.borrow_mut() = files_to_send;

        if model_item.endpoint_info().present.is_some() {
            let title = model_item
                .endpoint_info()
                .name
                .clone()
                .unwrap_or(gettext("Unknown device").into());
            model_item.set_device_name(title.clone());
        }

        let eta_estimator = &model_item.imp().eta;
        if eta_estimator.borrow().total_len == 0 {
            let total_size = imp
                .manage_files_model
                .iter::<gio::File>()
                .filter_map(|it| it.ok())
                .filter_map(|it| {
                    it.query_info(
                        gio::FILE_ATTRIBUTE_STANDARD_SIZE,
                        gio::FileQueryInfoFlags::NONE,
                        None::<&gio::Cancellable>,
                    )
                    .ok()
                })
                .map(|it| it.size() as usize)
                .fold(0, |acc, x| acc + x);

            eta_estimator
                .borrow_mut()
                .prepare_for_new_transfer(Some(total_size));
        }
    }

    // `card` style will be applied with `boxed-list*` on ListBox
    // v/h-align would prevent the card from expanding when space is available
    let root_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .margin_start(12)
        .margin_end(12)
        .margin_top(12)
        .margin_bottom(12)
        .spacing(12)
        .build();
    let root_bin = adw::Bin::builder().child(&root_box).build();
    // Animate the card sliding/fading in as the device is discovered.
    root_bin.add_css_class("device-found");

    let device_avatar = adw::Avatar::builder().show_initials(true).size(48).build();
    model_item
        .bind_property("device-name", &device_avatar, "text")
        .sync_create()
        .build();
    root_box.append(&device_avatar);

    let right_box = gtk::Box::builder().build();
    root_box.append(&right_box);

    let main_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .spacing(4)
        .build();
    right_box.append(&main_box);

    let title_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .css_classes(["title-4"])
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    model_item
        .bind_property("device-name", &title_label, "label")
        .sync_create()
        .build();
    let result_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .visible(false)
        .build();
    let unavailibility_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .label(&gettext("Unavailable"))
        .visible(false)
        .css_classes(["dimmed"])
        .build();
    let pincode_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .visible(false)
        .css_classes(["dimmed", "monospace"])
        .build();
    main_box.append(&title_label);
    main_box.append(&result_label);
    main_box.append(&unavailibility_label);
    main_box.append(&pincode_label);

    model_item.connect_transfer_state_notify(clone!(
        #[weak]
        imp,
        #[weak]
        result_label,
        move |model_item| {
            if model_item.transfer_state() == TransferState::Queued {
                result_label.set_visible(true);
                result_label.set_label(&gettext("Queued"));
                result_label.set_css_classes(&[]);
            };

            // Prevent exiting the recipients view until all transfers
            // are settled
            let is_transfer_active = imp
                .recipient_model
                .iter::<SendRequestState>()
                .filter_map(|it| it.ok())
                .find(|it| match it.transfer_state() {
                    TransferState::Queued
                    | TransferState::RequestedForConsent
                    | TransferState::OngoingTransfer => true,
                    TransferState::AwaitingConsentOrIdle
                    | TransferState::Failed
                    | TransferState::Done => false,
                })
                .is_some();
            if is_transfer_active {
                imp.select_recipients_dialog.set_can_close(false);
            } else {
                imp.select_recipients_dialog.set_can_close(true);
            }
        }
    ));

    // Circular progress with the percentage in the center (Quick Share style),
    // shown while a transfer is in progress.
    let circular = std::rc::Rc::new(widgets::create_circular_progress(56));
    circular.widget.set_visible(false);
    circular.widget.set_halign(gtk::Align::Start);
    circular.widget.set_margin_top(6);
    main_box.append(&circular.widget);

    let id = model_item.endpoint_info().id.clone();

    root_box.append(&adw::Bin::builder().hexpand(true).build());

    let retry_button = gtk::Button::builder()
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .icon_name("view-refresh-symbolic")
        .css_classes(["circular", "flat"])
        .tooltip_text(&gettext("Retry"))
        .visible(false)
        .build();
    root_box.append(&retry_button);
    retry_button.connect_clicked(clone!(
        #[weak]
        imp,
        #[weak]
        model_item,
        move |_button| {
            emit_send_files(&imp.obj(), &model_item);
        }
    ));

    let cancel_transfer_button = gtk::Button::builder()
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .icon_name("cross-large-symbolic")
        .css_classes(["circular", "flat"])
        .tooltip_text(&gettext("Cancel"))
        .visible(false)
        .build();
    root_box.append(&cancel_transfer_button);

    cancel_transfer_button.connect_clicked(clone!(
        #[weak(rename_to = rqs)]
        imp.rqs,
        #[strong]
        id,
        move |_button| {
            let mut guard = rqs.blocking_lock();
            if let Some(rqs) = guard.as_mut() {
                _ = rqs
                    .message_sender
                    .send(ChannelMessage {
                        id: id.clone(),
                        msg: rqs_lib::channel::Message::Lib {
                            action: rqs_lib::channel::TransferAction::TransferCancel,
                        },
                    })
                    .inspect_err(|err| tracing::error!(%err));
            }
        }
    ));

    fn transfer_fraction(client_msg: &MessageClient) -> Option<f64> {
        let metadata = client_msg.metadata.as_ref()?;
        if metadata.total_bytes > 0 {
            Some(metadata.ack_bytes as f64 / metadata.total_bytes as f64)
        } else {
            None
        }
    }

    fn set_row_activatable(
        model_item: &SendRequestState,
        row: Option<&gtk::ListBoxRow>,
        activatable: bool,
    ) {
        if let Some(row) = row {
            if model_item.endpoint_info().present.is_none() {
                row.set_activatable(false);
            } else {
                row.set_activatable(activatable);
            }
        }
    }

    model_item.connect_endpoint_info_notify(clone!(
        #[weak]
        win,
        #[weak]
        retry_button,
        #[weak]
        unavailibility_label,
        move |model_item| {
            let imp = win.imp();
            let is_idle_card = model_item.transfer_state() == TransferState::AwaitingConsentOrIdle;
            if let Some(row) = get_listbox_row_from_model_item::<SendRequestState>(
                &imp.recipient_model,
                &imp.recipient_listbox,
                model_item,
            ) {
                set_row_activatable(model_item, Some(&row), is_idle_card);
            };

            let endpoint_info = model_item.endpoint_info();
            if endpoint_info.present.is_none() {
                retry_button.set_sensitive(false);
                unavailibility_label.set_visible(is_idle_card);
            } else {
                retry_button.set_sensitive(true);
                unavailibility_label.set_visible(false);

                // Update device name on re-connection
                let title = endpoint_info
                    .name
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("Unknown Device");
                model_item.set_device_name(title);
            }
        }
    ));
    // Tracks a pending "consent timeout": if the peer never accepts (or
    // declines) within a few seconds of us requesting, we give up instead of
    // showing "Requested" forever. Mirrors how Google's client behaves.
    let consent_timeout_source: std::rc::Rc<
        std::cell::RefCell<Option<glib::SourceId>>,
    > = std::rc::Rc::new(std::cell::RefCell::new(None));

    model_item.connect_event_notify(clone!(
        #[weak]
        imp,
        #[strong]
        consent_timeout_source,
        #[strong]
        circular,
        move |model_item| {
            use rqs_lib::TransferState as RqsState;

            // Any new state transition cancels a pending consent timeout.
            if let Some(source) = consent_timeout_source.borrow_mut().take() {
                source.remove();
            }

            let eta_estimator = model_item.imp().eta.as_ref();

            if let Some(event_msg) = model_item.event() {
                let client_msg = event_msg.msg.as_client_unchecked();
                let state = client_msg.state.as_ref().unwrap_or(&RqsState::Initial);

                match state {
                    RqsState::Initial => {}
                    RqsState::ReceivedConnectionRequest => {}
                    RqsState::SentUkeyServerInit => {}
                    RqsState::SentPairedKeyEncryption => {}
                    RqsState::ReceivedUkeyClientFinish => {}
                    RqsState::SentConnectionResponse => {}
                    RqsState::SentPairedKeyResult => {}
                    RqsState::ReceivedPairedKeyResult => {}
                    RqsState::WaitingForUserConsent => {}
                    RqsState::ReceivingFiles => {}
                    RqsState::SentUkeyClientInit
                    | RqsState::SentUkeyClientFinish
                    | RqsState::SentIntroduction => {
                        model_item.set_transfer_state(TransferState::RequestedForConsent);

                        let listbox_row = get_listbox_row_from_model_item::<SendRequestState>(
                            &imp.recipient_model,
                            &imp.recipient_listbox,
                            model_item,
                        );
                        set_row_activatable(model_item, listbox_row.as_ref(), false);

                        unavailibility_label.set_visible(false);
                        retry_button.set_visible(false);

                        cancel_transfer_button.set_sensitive(true);
                        cancel_transfer_button.set_visible(true);

                        result_label.set_visible(true);
                        result_label.set_label(&gettext("Requested"));
                        result_label.set_css_classes(&["accent"]);

                        pincode_label.set_visible(true);
                        pincode_label.set_label(
                            &formatx!(
                                gettext("Code: {}"),
                                client_msg
                                    .metadata
                                    .as_ref()
                                    .map(|it| it.pin_code.as_ref().map(|it| it.as_str()))
                                    .flatten()
                                    .unwrap_or("???")
                            )
                            .unwrap_or_else(|_| "badly formatted locale string".into()),
                        );

                        eta_estimator.borrow_mut().prepare_for_new_transfer(None);

                        // Arm a consent timeout: if the peer doesn't move the
                        // transfer forward within 10s, mark it failed instead of
                        // hanging on "Requested" forever. Cancelled at the top of
                        // this closure on any subsequent state change.
                        let source = glib::timeout_add_seconds_local_once(
                            10,
                            clone!(
                                #[weak]
                                model_item,
                                #[weak]
                                result_label,
                                #[weak]
                                retry_button,
                                #[weak]
                                cancel_transfer_button,
                                #[weak]
                                pincode_label,
                                move || {
                                    if matches!(
                                        model_item.transfer_state(),
                                        TransferState::RequestedForConsent
                                    ) {
                                        model_item.set_transfer_state(TransferState::Failed);
                                        // Reflect the failure visually, like the
                                        // Disconnected branch does for the
                                        // remote-initiated case.
                                        cancel_transfer_button.set_visible(false);
                                        pincode_label.set_visible(false);
                                        retry_button.set_visible(true);
                                        result_label.set_visible(true);
                                        result_label.set_label(&gettext("No response"));
                                        result_label.set_css_classes(&["error"]);
                                    }
                                }
                            ),
                        );
                        consent_timeout_source.borrow_mut().replace(source);
                    }
                    RqsState::SendingFiles => {
                        model_item.set_transfer_state(TransferState::OngoingTransfer);

                        cancel_transfer_button.set_visible(true);
                        result_label.set_visible(false);
                        unavailibility_label.set_visible(false);
                        pincode_label.set_visible(false);
                        retry_button.set_visible(false);

                        // Circular progress with the percentage, animated.
                        circular.widget.set_visible(true);
                        if let Some(frac) = transfer_fraction(&client_msg) {
                            circular.set_fraction(frac);
                        }
                    }
                    RqsState::Disconnected => {
                        circular.widget.set_visible(false);
                        model_item.set_transfer_state(TransferState::Failed);
                        // FIXME: Wait for 5~10 seconds after a send and timeout
                        // if did not receive SendingFiles within that timeframe
                        // This is how google does it in their client

                        circular.widget.set_visible(false);
                        cancel_transfer_button.set_visible(false);
                        unavailibility_label.set_visible(false);
                        pincode_label.set_visible(false);

                        retry_button.set_visible(true);

                        result_label.set_visible(true);
                        result_label.set_label(&gettext("Failed"));
                        result_label.set_css_classes(&["error"]);
                    }
                    RqsState::Rejected => {
                        model_item.set_transfer_state(TransferState::Failed);
                        // Outbound(Reject) is not handled on lib side
                        // rqs_lib::hdl::outbound: Cannot process: consent denied: Reject
                    }
                    RqsState::Cancelled => {
                        model_item.set_transfer_state(TransferState::AwaitingConsentOrIdle);

                        let listbox_row = get_listbox_row_from_model_item::<SendRequestState>(
                            &imp.recipient_model,
                            &imp.recipient_listbox,
                            model_item,
                        );
                        set_row_activatable(model_item, listbox_row.as_ref(), true);

                        circular.widget.set_visible(false);
                        cancel_transfer_button.set_visible(false);
                        result_label.set_visible(false);
                        retry_button.set_visible(false);
                        pincode_label.set_visible(false);

                        unavailibility_label
                            .set_visible(model_item.endpoint_info().present.is_none());

                        model_item.set_event(None::<objects::ChannelMessage>);
                    }
                    RqsState::Finished => {
                        model_item.set_transfer_state(TransferState::Done);

                        cancel_transfer_button.set_visible(false);
                        // Snap to 100% then hide the ring on completion.
                        circular.set_fraction(1.0);
                        circular.widget.set_visible(false);
                        retry_button.set_visible(false);
                        unavailibility_label.set_visible(false);
                        pincode_label.set_visible(false);

                        let finished_text = {
                            let file_count = model_item.imp().files.borrow().len();
                            formatx!(
                                ngettext("Sent {} file", "Sent {} files", file_count as u32),
                                file_count
                            )
                            .unwrap_or_else(|_| "badly formatted locale string".into())
                        };

                        result_label.set_visible(true);
                        result_label.set_label(&finished_text);
                        // Celebratory pop on success.
                        result_label.set_css_classes(&["accent", "transfer-done"]);

                        // Also surface a toast so success is noticed even if the
                        // recipients dialog isn't in focus.
                        imp.toast_overlay.add_toast(
                            adw::Toast::builder()
                                .title(
                                    &formatx!(
                                        gettext("Sent to {}"),
                                        model_item.device_name()
                                    )
                                    .unwrap_or_else(|_| finished_text.clone()),
                                )
                                .timeout(3)
                                .build(),
                        );
                    }
                };
            }
        }
    ));

    // Set initial widget state based on model's state
    model_item.notify_endpoint_info();
    model_item.notify_event();

    root_bin
}
