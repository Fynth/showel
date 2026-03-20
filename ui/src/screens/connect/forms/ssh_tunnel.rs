use dioxus::prelude::*;

#[component]
pub fn SshTunnelFields(
    enabled: Signal<bool>,
    host: Signal<String>,
    port: Signal<String>,
    username: Signal<String>,
    private_key_path: Signal<String>,
) -> Element {
    rsx! {
        div {
            class: "connect-form__ssh",
            div {
                class: "connect-form__ssh-header",
                div {
                    p { class: "connect-screen__section-title", "SSH Tunnel" }
                    p {
                        class: "connect-screen__status connect-screen__status--hint",
                        "Forward the database port through the local OpenSSH client using agent or private key authentication."
                    }
                }
                button {
                    class: if enabled() {
                        "button button--ghost button--small button--active"
                    } else {
                        "button button--ghost button--small"
                    },
                    onclick: move |_| enabled.toggle(),
                    if enabled() { "Disable SSH" } else { "Enable SSH" }
                }
            }

            if enabled() {
                div {
                    class: "connect-form__grid connect-form__ssh-grid",
                    div {
                        class: "field",
                        label { class: "field__label", r#for: "ssh-host", "SSH Host" }
                        input {
                            class: "input",
                            id: "ssh-host",
                            value: "{host}",
                            placeholder: "bastion.example.com",
                            oninput: move |event| host.set(event.value()),
                        }
                    }
                    div {
                        class: "field",
                        label { class: "field__label", r#for: "ssh-port", "SSH Port" }
                        input {
                            class: "input",
                            id: "ssh-port",
                            value: "{port}",
                            placeholder: "22",
                            oninput: move |event| port.set(event.value()),
                        }
                    }
                }

                div {
                    class: "connect-form__grid connect-form__ssh-grid",
                    div {
                        class: "field",
                        label { class: "field__label", r#for: "ssh-username", "SSH Username" }
                        input {
                            class: "input",
                            id: "ssh-username",
                            value: "{username}",
                            placeholder: "ubuntu",
                            oninput: move |event| username.set(event.value()),
                        }
                    }
                    div {
                        class: "field",
                        label { class: "field__label", r#for: "ssh-key", "Private Key Path" }
                        input {
                            class: "input",
                            id: "ssh-key",
                            value: "{private_key_path}",
                            placeholder: "~/.ssh/id_ed25519 (optional if agent is configured)",
                            oninput: move |event| private_key_path.set(event.value()),
                        }
                    }
                }
            }
        }
    }
}
