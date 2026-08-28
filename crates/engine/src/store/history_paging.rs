use super::*;

impl Store {
    pub fn history_page(&self, offset: u64, limit: u32) -> Result<HistoryPage> {
        let offset = i64::try_from(offset).map_err(|_| anyhow!("历史分页位置超出范围"))?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let total = transaction.query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
        let items = {
            let mut statement = transaction
                .prepare("SELECT data FROM history ORDER BY time DESC,id ASC LIMIT ?1 OFFSET ?2")?;
            let rows = statement.query_map(params![limit.clamp(1, 100), offset], |row| {
                row.get::<_, String>(0)
            })?;
            rows.map(|row| Ok(serde_json::from_str(&row?)?))
                .collect::<Result<Vec<HistoryItem>>>()?
        };
        transaction.commit()?;
        Ok(HistoryPage { items, total })
    }
}

#[cfg(test)]
mod tests;
