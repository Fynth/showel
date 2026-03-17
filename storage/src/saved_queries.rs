use models::SavedQuery;

use crate::fs_store::{read_json_file, saved_queries_path, write_json_file};

pub async fn load_saved_queries() -> Result<Vec<SavedQuery>, String> {
    let mut items: Vec<SavedQuery> = read_json_file(saved_queries_path()).await?;
    items.sort_by(|left, right| {
        left.folder_name()
            .cmp(right.folder_name())
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(items)
}

pub async fn save_saved_query(item: SavedQuery) -> Result<(), String> {
    let mut items = load_saved_queries().await.unwrap_or_default();
    items.retain(|existing| existing.id != item.id);
    items.push(item);
    write_json_file(saved_queries_path(), &items).await
}

pub async fn delete_saved_query(id: u64) -> Result<(), String> {
    let mut items = load_saved_queries().await.unwrap_or_default();
    items.retain(|existing| existing.id != id);
    write_json_file(saved_queries_path(), &items).await
}
