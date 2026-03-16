use dioxus::prelude::*;
pub mod db_connect;
#[component]
pub fn App() -> Element {
    let connection = use_signal(|| None);
    if connection().is_some() {
        rsx! { Workspace {} }
    } else {
        rsx! { db_connect::DbConnect {} }
    }
}
