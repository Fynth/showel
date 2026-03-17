use crate::{
    app_state::{APP_STATE, APP_THEME},
    layout::{StatusBar, Toolbar},
    screens::{DbConnect, Workspace},
};
use dioxus::prelude::*;

static APP_CSS: Asset = asset!("/assets/app.css");

#[component]
pub fn App() -> Element {
    let theme_name = APP_THEME();
    let (has_sessions, should_show_connect) = {
        let app_state = APP_STATE.read();
        (
            app_state.has_sessions(),
            app_state.show_connection_screen || !app_state.has_sessions(),
        )
    };

    rsx! {
        document::Stylesheet {
            href: APP_CSS,
        }

        div {
            class: "app {theme_name}",
            Toolbar {}
            main {
                class: if has_sessions {
                    "app__body"
                } else {
                    "app__body app__body--welcome"
                },
                if has_sessions {
                    Workspace {}
                    if should_show_connect {
                        div {
                            class: "app__overlay",
                            DbConnect {}
                        }
                    }
                } else {
                    DbConnect {}
                }
            }
            StatusBar {}
        }
    }
}
