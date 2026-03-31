use serde::{Deserialize, Serialize};

/// A cached semantic query entry containing an embedding and its associated response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticCacheEntry {
    /// Unique identifier for this cache entry.
    pub id: String,
    /// The normalized query string.
    pub query_normalized: String,
    /// The 384-dimensional embedding vector for the query.
    pub embedding: Vec<f32>,
    /// The cached response text.
    pub response_text: String,
    /// Unix timestamp (seconds) when this entry was created.
    pub created_at: i64,
    /// Number of times this cache entry has been accessed.
    pub hit_count: u64,
}

/// Configuration for the semantic cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticCacheConfig {
    /// Minimum cosine similarity score (0.0 to 1.0) for a cache hit.
    /// Default: 0.90
    pub similarity_threshold: f32,
    /// Maximum number of entries to store in the cache.
    pub max_entries: usize,
    /// Time-to-live for cache entries in seconds.
    pub ttl_seconds: u64,
}

impl Default for SemanticCacheConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.90,
            max_entries: 1000,
            ttl_seconds: 3600, // 1 hour
        }
    }
}

/// Result of a semantic cache lookup.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CacheLookupResult {
    /// Cache hit with the similarity score and cached response.
    Hit {
        /// Cosine similarity score between the query and cached entry.
        similarity_score: f32,
        /// The cached response text.
        response: String,
    },
    /// Cache miss - no matching entry found.
    Miss,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_cache_entry_creation() {
        let entry = SemanticCacheEntry {
            id: "test-id".to_string(),
            query_normalized: "select * from users".to_string(),
            embedding: vec![0.1; 384],
            response_text: "SELECT * FROM users;".to_string(),
            created_at: 1234567890,
            hit_count: 5,
        };

        assert_eq!(entry.id, "test-id");
        assert_eq!(entry.hit_count, 5);
        assert_eq!(entry.embedding.len(), 384);
    }

    #[test]
    fn test_semantic_cache_config_default() {
        let config = SemanticCacheConfig::default();

        assert_eq!(config.similarity_threshold, 0.90);
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_seconds, 3600);
    }

    #[test]
    fn test_cache_lookup_result_hit() {
        let result = CacheLookupResult::Hit {
            similarity_score: 0.95,
            response: "Cached response".to_string(),
        };

        match result {
            CacheLookupResult::Hit {
                similarity_score,
                response,
            } => {
                assert_eq!(similarity_score, 0.95);
                assert_eq!(response, "Cached response");
            }
            CacheLookupResult::Miss => panic!("Expected Hit variant"),
        }
    }

    #[test]
    fn test_cache_lookup_result_miss() {
        let result = CacheLookupResult::Miss;

        match result {
            CacheLookupResult::Miss => {}
            CacheLookupResult::Hit { .. } => panic!("Expected Miss variant"),
        }
    }
}
