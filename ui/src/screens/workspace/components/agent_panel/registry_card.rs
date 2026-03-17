use dioxus::prelude::*;
use models::AcpRegistryAgent;

#[component]
pub(super) fn RegistryAgentCard(
    agent: AcpRegistryAgent,
    busy: bool,
    on_connect: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        article { class: "agent-panel__registry-card",
            div { class: "agent-panel__registry-copy",
                div { class: "agent-panel__registry-row",
                    h5 { class: "agent-panel__registry-title", "{agent.name}" }
                    span { class: "agent-panel__badge", "v{agent.version}" }
                }
                p { class: "agent-panel__hint", "{agent.description}" }
                p {
                    class: "agent-panel__hint",
                    if agent.installed {
                        "Installed locally and ready to connect."
                    } else {
                        "Downloads and starts the official registry build as `opencode acp`."
                    }
                }
            }
            button {
                class: "button button--primary button--small",
                disabled: busy,
                onclick: move |event| on_connect.call(event),
                if busy { "Preparing..." } else if agent.installed { "Connect OpenCode" } else { "Install & Connect OpenCode" }
            }
        }
    }
}
