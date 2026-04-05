use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
#[allow(dead_code)]
pub enum BlobViewMode {
    Hex,
    Text,
    Image,
}

#[derive(Clone, PartialEq)]
pub struct BlobData {
    pub raw: Vec<u8>,
    pub mime_type: Option<String>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct HexLine {
    pub address: String,
    pub bytes: Vec<HexByte>,
    pub ascii: String,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct HexByte {
    pub hex: String,
    pub char: char,
    pub is_printable: bool,
}

#[component]
pub fn BlobViewer(mut blob_data: Signal<Option<BlobData>>, on_close: Callback<()>) -> Element {
    let mut view_mode = use_signal(|| BlobViewMode::Hex);
    let mut selected_offset = use_signal(|| 0u64);
    let bytes_per_line = 16;

    let data = blob_data();
    let Some(blob) = data else {
        return rsx! {
            div {
                class: "blob-viewer blob-viewer--empty",
                div {
                    class: "blob-viewer__header",
                    span { class: "blob-viewer__title", "BLOB Viewer" }
                    button {
                        class: "blob-viewer__close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                div {
                    class: "blob-viewer__empty-state",
                    "No data to display"
                }
            }
        };
    };

    let total_size = blob.raw.len() as u64;
    let suggested_mode = detect_blob_type(&blob.raw, blob.mime_type.as_deref());
    if view_mode() == BlobViewMode::Hex && suggested_mode != BlobViewMode::Hex {
        view_mode.set(suggested_mode);
    }

    let hex_lines = render_hex_dump(&blob.raw, bytes_per_line);
    let text_content = render_text_preview(&blob.raw);
    let image_data_url = render_image_preview(&blob.raw);

    let max_offset = (total_size.saturating_sub(1) / bytes_per_line as u64) * bytes_per_line as u64;

    rsx! {
        div {
            class: "blob-viewer",
            div {
                class: "blob-viewer__header",
                span {
                    class: "blob-viewer__title",
                    "BLOB Viewer — {format_bytes(total_size)}"
                }
                div {
                    class: "blob-viewer__tabs",
                    button {
                        class: if view_mode() == BlobViewMode::Hex { "active" },
                        onclick: move |_| view_mode.set(BlobViewMode::Hex),
                        "Hex"
                    }
                    button {
                        class: if view_mode() == BlobViewMode::Text { "active" },
                        onclick: move |_| view_mode.set(BlobViewMode::Text),
                        "Text"
                    }
                    if image_data_url.is_some() {
                        button {
                            class: if view_mode() == BlobViewMode::Image { "active" },
                            onclick: move |_| view_mode.set(BlobViewMode::Image),
                            "Image"
                        }
                    }
                }
                button {
                    class: "blob-viewer__close",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            div {
                class: "blob-viewer__content",
                match view_mode() {
                    BlobViewMode::Hex => rsx! {
                        div {
                            class: "blob-viewer__hex-view",
                            div {
                                class: "blob-viewer__hex-nav",
                                button {
                                    disabled: selected_offset() == 0,
                                    onclick: move |_| selected_offset.set(0),
                                    "Top"
                                }
                                button {
                                    disabled: selected_offset() == 0,
                                    onclick: move |_| selected_offset.set(selected_offset().saturating_sub(256)),
                                    "-256"
                                }
                                button {
                                    disabled: selected_offset() >= max_offset,
                                    onclick: move |_| selected_offset.set(std::cmp::min(selected_offset() + 256, max_offset)),
                                    "+256"
                                }
                                button {
                                    disabled: selected_offset() >= max_offset,
                                    onclick: move |_| selected_offset.set(max_offset),
                                    "Bottom"
                                }
                                span {
                                    class: "blob-viewer__offset",
                                    "Offset: {selected_offset()}"
                                }
                            }
                            pre {
                                class: "blob-viewer__hex-dump",
                                code {
                                    for line in hex_lines.iter() {
                                        span {
                                            class: "blob-viewer__hex-line",
                                            span {
                                                class: "blob-viewer__hex-address",
                                                "{line.address}"
                                            }
                                            span {
                                                class: "blob-viewer__hex-bytes",
                                                for byte in line.bytes.iter() {
                                                    if byte.is_printable {
                                                        span {
                                                            class: "blob-viewer__hex-char--printable",
                                                            "{byte.hex}"
                                                        }
                                                    } else {
                                                        span {
                                                            class: "blob-viewer__hex-char--binary",
                                                            "{byte.hex}"
                                                        }
                                                    }
                                                    " "
                                                }
                                                if line.bytes.len() < 8 {
                                                    "  "
                                                }
                                                span {
                                                    class: "blob-viewer__hex-ascii",
                                                    "{line.ascii}"
                                                }
                                            }
                                            "\n"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    BlobViewMode::Text => rsx! {
                        div {
                            class: "blob-viewer__text-view",
                            pre {
                                class: "blob-viewer__text-content",
                                "{text_content}"
                            }
                        }
                    },
                    BlobViewMode::Image => rsx! {
                        div {
                            class: "blob-viewer__image-view",
                            if let Some(data_url) = image_data_url {
                                img {
                                    src: "{data_url}",
                                    alt: "BLOB Image Preview"
                                }
                            }
                        }
                    }
                }
            }
            div {
                class: "blob-viewer__footer",
                span {
                    class: "blob-viewer__info",
                    if let Some(mime) = blob.mime_type.as_ref() {
                        "Type: {mime}"
                    } else {
                        "Type: binary"
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
fn detect_blob_type(data: &[u8], mime_hint: Option<&str>) -> BlobViewMode {
    if let Some(mime) = mime_hint {
        if mime.starts_with("image/") {
            return BlobViewMode::Image;
        }
        if mime.starts_with("text/") || mime.contains("xml") || mime.contains("json") {
            return BlobViewMode::Text;
        }
    }

    if data.len() >= 4 {
        match [data[0], data[1], data[2], data[3]] {
            [0x89, 0x50, 0x4E, 0x47] => return BlobViewMode::Image,
            [0xFF, 0xD8, 0xFF, _] => return BlobViewMode::Image,
            [0x47, 0x49, 0x46, _] => return BlobViewMode::Image,
            [0x52, 0x49, 0x46, 0x46] => return BlobViewMode::Image,
            [0x42, 0x4D, _, _] => return BlobViewMode::Image,
            _ => {}
        }
    }

    if data.len() >= 5 && (data.starts_with(b"<?xml") || data.starts_with(b"<svg")) {
        return BlobViewMode::Text;
    }

    if data.len() >= 6 {
        let header_lower = String::from_utf8_lossy(&data[..6]).to_lowercase();
        if header_lower.contains("html") || header_lower.contains("doctype") {
            return BlobViewMode::Text;
        }
    }

    BlobViewMode::Hex
}

#[allow(dead_code)]
fn render_hex_dump(data: &[u8], bytes_per_line: usize) -> Vec<HexLine> {
    data.chunks(bytes_per_line)
        .enumerate()
        .map(|(line_offset, chunk)| {
            let address = format!("{:08x}:", line_offset * bytes_per_line);
            let bytes: Vec<HexByte> = chunk
                .iter()
                .map(|&b| {
                    let is_printable = (0x20..0x7f).contains(&b);
                    let char = if is_printable { b as char } else { '.' };
                    HexByte {
                        hex: format!("{:02x}", b),
                        char,
                        is_printable,
                    }
                })
                .collect();
            let ascii: String = bytes.iter().map(|b| b.char).collect();
            HexLine {
                address,
                bytes,
                ascii,
            }
        })
        .collect()
}

#[allow(dead_code)]
fn render_text_preview(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

#[allow(dead_code)]
fn render_image_preview(data: &[u8]) -> Option<String> {
    let mime = if data.len() >= 4 {
        match [data[0], data[1], data[2], data[3]] {
            [0x89, 0x50, 0x4E, 0x47] => "image/png",
            [0xFF, 0xD8, 0xFF, _] => "image/jpeg",
            [0x47, 0x49, 0x46, _] => "image/gif",
            [0x52, 0x49, 0x46, 0x46] => "image/webp",
            _ => return None,
        }
    } else {
        return None;
    };

    let base64 = base64_encode(data);
    Some(format!("data:{mime};base64,{base64}"))
}

#[allow(dead_code)]
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        result.push(ALPHABET[(b[0] >> 2) as usize] as char);
        result.push(ALPHABET[((b[0] & 0x03) << 4 | b[1] >> 4) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((b[1] & 0x0f) << 2 | b[2] >> 6) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(b[2] & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[allow(dead_code)]
fn format_bytes(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{size} bytes")
    }
}
