use dioxus::prelude::*;
pub mod db_connect;
#[component]
pub fn App() -> Element {
    rsx! {
        db_connect::DbConnect {}
    }
}
