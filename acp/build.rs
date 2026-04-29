fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(feature = "embedding")]
    {
        const MODEL_URL: &str = "https://huggingface.co/Ayeshas21/sentence-transformers-all-MiniLM-L6-v2-quantized/resolve/main/model-quant.onnx";
        const MODEL_FILENAME: &str = "all-MiniLM-L6-v2-INT8.onnx";

        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        let model_path = std::path::Path::new(&out_dir).join(MODEL_FILENAME);

        if model_path.exists() {
            let metadata = std::fs::metadata(&model_path).expect("Failed to read model metadata");
            if metadata.len() > 0 {
                println!("cargo:warning=Model already exists at {:?}", model_path);
                return;
            }
        }

        println!("cargo:warning=Downloading embedding model from HuggingFace...");

        match download_model(&model_path, MODEL_URL) {
            Ok(_) => println!(
                "cargo:warning=Model downloaded successfully to {:?}",
                model_path
            ),
            Err(e) => panic!("Failed to download model: {}", e),
        }
    }
}

#[cfg(feature = "embedding")]
fn download_model(dest_path: &std::path::Path, model_url: &str) -> Result<(), String> {
    let output = std::process::Command::new("curl")
        .args(["-L", "-o", &dest_path.to_string_lossy(), model_url])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl failed: {}", stderr));
    }

    Ok(())
}
