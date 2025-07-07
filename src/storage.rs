use crate::models::Client;
use chrono::Local;
use std::collections::HashMap;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

pub async fn persist(
    clients: &HashMap<Uuid, Client>,
    file_counter: u32,
) -> std::io::Result<String> {
    let date_str = Local::now().format("%d%m%Y").to_string();
    let filename = format!("{date_str}_{file_counter}.DAT");

    let mut file = File::create(&filename).await?;
    for (id, client) in clients {
        let line = format!("{id} {}\n", client.balance);
        file.write_all(line.as_bytes()).await?;
    }
    file.flush().await?;
    Ok(filename)
}