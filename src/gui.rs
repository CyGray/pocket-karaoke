//! The Slint UI runs on the main thread. Network capture runs on a background
//! thread and the WASAPI callback renders audio independently. Polling occurs
//! only five times a second to avoid unnecessary work.

use pocket_karaoke::desktop::Receiver;
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::time::Duration;

slint::include_modules!();

type GuiResult<T> = Result<T, Box<dyn Error>>;

pub fn run() -> GuiResult<()> {
    let ui = AppWindow::new()?;
    let receiver: Rc<RefCell<Option<Receiver>>> = Rc::new(RefCell::new(None));

    let gain_ref = receiver.clone();
    let gain_ui = ui.as_weak();
    ui.on_gain_changed(move |percent| {
        let value = percent.clamp(0.0, 200.0);
        if let Some(receiver) = gain_ref.borrow().as_ref() {
            receiver.set_gain(value / 100.0);
        }
        if let Some(ui) = gain_ui.upgrade() {
            ui.set_gain_label(format!("{}%", value.round() as i32).into());
        }
    });

    let buffer_ui = ui.as_weak();
    ui.on_buffer_changed(move |ms| {
        if let Some(ui) = buffer_ui.upgrade() {
            ui.set_buffer_label(format!("{} ms", ms.round() as i32).into());
        }
    });

    let start_ref = receiver.clone();
    let start_ui = ui.as_weak();
    ui.on_toggle_listening(move || {
        let Some(ui) = start_ui.upgrade() else { return };
        let mut current = start_ref.borrow_mut();
        if current.is_some() {
            *current = None;
            ui.set_listener_active(false);
            ui.set_phone_connected(false);
            ui.set_pin_code("------".into());
            ui.set_connection_status("Receiver stopped".into());
            ui.set_connection_note("Stopped. The phone connection will close.".into());
            ui.set_error_message("".into());
            return;
        }
        let wifi = ui.get_wifi_mode();
        let buffer_ms = (ui.get_buffer_value().round() as u32).clamp(20, 100);
        let gain = (ui.get_gain_value() / 100.0).clamp(0.0, 2.0);
        match Receiver::start(wifi, 8765, buffer_ms, gain) {
            Ok(next) => {
                ui.set_pin_code(next.pin().into());
                ui.set_endpoint(next.endpoint().into());
                ui.set_audio_device(next.device_name().into());
                ui.set_listener_active(true);
                ui.set_phone_connected(false);
                ui.set_connection_status("Waiting for your phone".into());
                ui.set_connection_note(if wifi {
                    "Connect your phone to the same trusted Wi-Fi; enter the host and PIN.".into()
                } else {
                    "Run adb reverse tcp:8765 tcp:8765, then enter the pairing PIN.".into()
                });
                ui.set_error_message("".into());
                *current = Some(next);
            }
            Err(e) => {
                ui.set_connection_status("Could not start receiver".into());
                ui.set_error_message(format!("{e}").into());
            }
        }
    });

    ui.on_open_youtube(|| {
        // Arguments are fixed, trusted strings: no user-input shell expansion.
        let _ = std::process::Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                "https://www.youtube.com/results?search_query=karaoke+instrumental",
            ])
            .spawn();
    });

    // Slint's timer remains in scope for the entire event loop.
    let status_ref = receiver.clone();
    let status_ui = ui.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        if let (Some(ui), Some(receiver)) = (status_ui.upgrade(), status_ref.borrow().as_ref()) {
            let connected = receiver.connected();
            if connected != ui.get_phone_connected() {
                ui.set_phone_connected(connected);
                ui.set_connection_status(if connected {
                    "Microphone live — you're on stage".into()
                } else {
                    "Waiting for your phone".into()
                });
            }
        }
    });

    ui.run()?;
    drop(timer);
    *receiver.borrow_mut() = None;
    Ok(())
}
