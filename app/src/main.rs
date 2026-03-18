#![cfg_attr(
    all(feature = "bundle", target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus::{
    LaunchBuilder,
    desktop::{Config, LogicalSize, WindowBuilder, tao::event_loop::EventLoopBuilder},
    prelude::*,
};
use rfd::{MessageButtons, MessageDialog, MessageLevel};
use std::{
    backtrace::Backtrace,
    fs,
    panic::{self, PanicHookInfo},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use ui::App as UiApp;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use dioxus::desktop::tao::platform::unix::EventLoopBuilderExtUnix;

const APP_CSS: &str = include_str!("../assets/app.css");

fn main() {
    install_crash_reporter();

    if panic::catch_unwind(launch_app).is_err() {
        std::process::exit(1);
    }
}

fn launch_app() {
    let mut event_loop_builder = EventLoopBuilder::with_user_event();
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    event_loop_builder.with_app_id("dev.showel.app");
    let event_loop = event_loop_builder.build();

    LaunchBuilder::desktop()
        .with_cfg(
            Config::new()
                .with_event_loop(event_loop)
                .with_menu(None)
                .with_disable_context_menu(true)
                .with_disable_dma_buf_on_wayland(true)
                .with_window(
                    WindowBuilder::new()
                        .with_title("Showel")
                        .with_inner_size(LogicalSize::new(1440.0, 920.0))
                        .with_min_inner_size(LogicalSize::new(720.0, 480.0))
                        .with_always_on_top(false)
                        .with_resizable(true),
                ),
        )
        .launch(Root);
}

#[component]
fn Root() -> Element {
    rsx! {
        style {
            {APP_CSS}
        }
        UiApp {}
    }
}

fn install_crash_reporter() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let report = build_panic_report(info);
        let log_path = write_crash_report(&report);

        default_hook(info);
        eprintln!("{report}");

        let description = match log_path {
            Some(path) => format!("{report}\n\nCrash log:\n{}", path.display()),
            None => report,
        };

        show_error_dialog("Showel failed to start", &description);
    }));
}

fn build_panic_report(info: &PanicHookInfo<'_>) -> String {
    let location = info
        .location()
        .map(|location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        })
        .unwrap_or_else(|| "unknown location".to_string());
    let backtrace = Backtrace::force_capture();

    format!(
        "Showel panicked.\n\nMessage: {}\nLocation: {location}\n\nBacktrace:\n{backtrace}",
        panic_message(info)
    )
}

fn panic_message(info: &PanicHookInfo<'_>) -> String {
    if let Some(message) = info.payload().downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "panic payload is not a string".to_string()
    }
}

fn write_crash_report(report: &str) -> Option<PathBuf> {
    let mut log_dir = std::env::temp_dir();
    log_dir.push("showel");
    fs::create_dir_all(&log_dir).ok()?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let path = log_dir.join(format!("crash-{timestamp}.log"));
    fs::write(&path, report).ok()?;
    Some(path)
}

fn show_error_dialog(title: &str, description: &str) {
    let _ = MessageDialog::new()
        .set_title(title)
        .set_description(description)
        .set_level(MessageLevel::Error)
        .set_buttons(MessageButtons::Ok)
        .show();
}
