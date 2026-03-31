use ort::session::Session;
use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

/// Default model filename for the INT8 quantized all-MiniLM-L6-v2 model
pub const MODEL_FILENAME: &str = "all-MiniLM-L6-v2-INT8.onnx";

/// Default tokenizer filename
pub const TOKENIZER_FILENAME: &str = "tokenizer.json";

/// Embedding dimension for all-MiniLM-L6-v2
pub const EMBEDDING_DIM: usize = 384;

/// Maximum sequence length for the model
pub const MAX_SEQ_LENGTH: usize = 512;

/// Embedding model that wraps an ONNX Runtime session and tokenizer
#[derive(Clone)]
pub struct EmbeddingModel {
    session: Arc<Mutex<Session>>,
    tokenizer: Arc<Tokenizer>,
}

impl EmbeddingModel {
    /// Load the embedding model from the given directory path
    /// 
    /// # Arguments
    /// * `model_dir` - Directory containing the ONNX model and tokenizer files
    /// 
    /// # Returns
    /// * `Ok(EmbeddingModel)` - If model and tokenizer loaded successfully
    /// * `Err(String)` - If loading failed
    pub fn load_model(model_dir: &str) -> Result<Self, String> {
        let model_path = std::path::Path::new(model_dir).join(MODEL_FILENAME);
        let tokenizer_path = std::path::Path::new(model_dir).join(TOKENIZER_FILENAME);

        Self::load_model_from_paths(&model_path, &tokenizer_path)
    }

    /// Load the embedding model from specific file paths
    /// 
    /// # Arguments
    /// * `model_path` - Path to the ONNX model file
    /// * `tokenizer_path` - Path to the tokenizer.json file
    /// 
    /// # Returns
    /// * `Ok(EmbeddingModel)` - If model and tokenizer loaded successfully
    /// * `Err(String)` - If loading failed
    pub fn load_model_from_paths(
        model_path: &std::path::Path,
        tokenizer_path: &std::path::Path,
    ) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load model from {:?}: {}", model_path, e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e))?;

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            tokenizer: Arc::new(tokenizer),
        })
    }

    /// Generate an embedding for the given text synchronously
    /// 
    /// This method performs CPU-intensive ONNX inference and should be called
    /// within spawn_blocking to avoid blocking the async runtime.
    /// 
    /// # Arguments
    /// * `text` - The text to embed
    /// 
    /// # Returns
    /// * `Ok(Vec<f32>)` - 384-dimensional normalized embedding vector
    /// * `Err(String)` - If embedding generation failed
    pub fn embed_sync(&self, text: &str) -> Result<Vec<f32>, String> {
        let truncated_text = if text.len() > MAX_SEQ_LENGTH * 4 {
            &text[..MAX_SEQ_LENGTH * 4]
        } else {
            text
        };

        let encoding = self
            .tokenizer
            .encode(truncated_text, true)
            .map_err(|e| format!("Tokenization failed: {}", e))?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let seq_len = input_ids.len();

        if seq_len == 0 {
            return Err("Empty token sequence".to_string());
        }

        let input_ids_i64: Vec<i64> = input_ids.iter().map(|&x| x as i64).collect();
        let attention_mask_i64: Vec<i64> = attention_mask.iter().map(|&x| x as i64).collect();
        let token_type_ids = vec![0i64; seq_len];

        // Create ORT tensors using (shape, data) tuples
        let input_ids_tensor = ort::value::Tensor::from_array((vec![1i64, seq_len as i64], input_ids_i64))
            .map_err(|e| format!("Failed to create input_ids tensor: {}", e))?;
        let attention_mask_tensor = ort::value::Tensor::from_array((vec![1i64, seq_len as i64], attention_mask_i64))
            .map_err(|e| format!("Failed to create attention_mask tensor: {}", e))?;
        let token_type_ids_tensor = ort::value::Tensor::from_array((vec![1i64, seq_len as i64], token_type_ids))
            .map_err(|e| format!("Failed to create token_type_ids tensor: {}", e))?;

        let mut session_guard = self
            .session
            .lock()
            .map_err(|e| format!("Failed to lock session: {}", e))?;
        let outputs = session_guard
            .run(ort::inputs![input_ids_tensor, attention_mask_tensor, token_type_ids_tensor])
            .map_err(|e| format!("ONNX inference failed: {}", e))?;

        let (shape, embeddings_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Failed to extract embeddings: {}", e))?;

        let _batch_size = shape[0] as usize;
        let seq_len_out = shape[1] as usize;
        let _embedding_dim = shape[2] as usize;

        let mut pooled = vec![0.0f32; EMBEDDING_DIM];
        for token_idx in 0..seq_len_out {
            for dim_idx in 0..EMBEDDING_DIM {
                pooled[dim_idx] += embeddings_data[token_idx * EMBEDDING_DIM + dim_idx];
            }
        }

        for val in pooled.iter_mut() {
            *val /= seq_len_out as f32;
        }

        normalize_vector(&mut pooled);

        Ok(pooled)
    }

    /// Generate an embedding for the given text asynchronously
    /// 
    /// This method uses spawn_blocking to run the CPU-intensive ONNX inference
    /// on a separate thread pool, preventing blocking of the async runtime.
    /// Target: <200ms for embed() call
    /// 
    /// # Arguments
    /// * `text` - The text to embed
    /// 
    /// # Returns
    /// * `Ok(Vec<f32>)` - 384-dimensional normalized embedding vector
    /// * `Err(String)` - If embedding generation failed
    pub async fn embed(&self, text: String) -> Result<Vec<f32>, String> {
        let model = self.clone();

        tokio::task::spawn_blocking(move || model.embed_sync(&text))
            .await
            .map_err(|e| format!("Embedding task panicked: {}", e))?
    }
}

fn normalize_vector(vec: &mut [f32]) {
    let sum_squares: f32 = vec.iter().map(|x| x * x).sum();
    if sum_squares > 0.0 {
        let norm = sum_squares.sqrt();
        for val in vec.iter_mut() {
            *val /= norm;
        }
    }
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> f32 {
    if vec1.len() != vec2.len() {
        return 0.0;
    }

    let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = vec1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm2: f32 = vec2.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm1 > 0.0 && norm2 > 0.0 {
        dot_product / (norm1 * norm2)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let vec = vec![1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&vec, &vec);
        assert!((similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![0.0, 1.0, 0.0];
        let similarity = cosine_similarity(&vec1, &vec2);
        assert!(similarity.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![-1.0, 0.0, 0.0];
        let similarity = cosine_similarity(&vec1, &vec2);
        assert!((similarity - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_normalize_vector() {
        let mut vec = vec![3.0, 4.0];
        normalize_vector(&mut vec);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_normalize_already_normalized() {
        let mut vec = vec![1.0, 0.0, 0.0];
        normalize_vector(&mut vec);
        assert!((vec[0] - 1.0).abs() < 0.001);
        assert!(vec[1].abs() < 0.001);
        assert!(vec[2].abs() < 0.001);
    }
}
