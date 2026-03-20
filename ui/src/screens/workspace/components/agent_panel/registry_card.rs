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
            div { class: "agent-panel__registry-row",
                div { class: "agent-panel__registry-copy",
                    h5 { class: "agent-panel__registry-title", "{agent.name}" }
                    p { class: "agent-panel__hint", "{agent.description}" }
                }
                span { class: "agent-panel__badge", "v{agent.version}" }
            }
            div { class: "agent-panel__registry-actions",
                p {
                    class: "agent-panel__hint agent-panel__hint--status",
                    if agent.installed {
                        "Installed locally"
                    } else {
                        "Installs on first connect"
                    }
                }
                button {
                    class: "button button--primary button--small",
                    disabled: busy,
                    onclick: move |event| on_connect.call(event),
                    if busy {
                        "Preparing..."
                    } else if agent.installed {
                        "Connect"
                    } else {
                        "Install & connect"
                    }
                }
            }
        }
    }
}
