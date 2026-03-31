use ort::session::Session;

pub struct EmbeddingModel {
    session: Session,
}

impl EmbeddingModel {
    pub fn load_model(path: &str) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?
            .commit_from_file(path)
            .map_err(|e| format!("Failed to load model from {}: {}", path, e))?;

        Ok(Self { session })
    }

    pub fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
        Ok(vec![])
    }
}

pub const MODEL_FILENAME: &str = "all-MiniLM-L6-v2-INT8.onnx";
