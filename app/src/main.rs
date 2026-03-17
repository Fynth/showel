#![cfg_attr(
    all(feature = "bundle", target_os = "windows"),
    windows_subsystem = "windows"
)]

use dioxus::{
    LaunchBuilder,
    desktop::{Config, LogicalSize, WindowBuilder, tao::event_loop::EventLoopBuilder},
};
use ui::App;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use dioxus::desktop::tao::platform::unix::EventLoopBuilderExtUnix;

fn main() {
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
        .launch(App);
}
