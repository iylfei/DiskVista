use super::*;

impl Store {
    pub fn history_page(&self, offset: u64, limit: u32) -> Result<HistoryPage> {
        self.history_page_filtered(offset, limit, false)
    }

    pub fn recycled_history_page(&self, offset: u64, limit: u32) -> Result<HistoryPage> {
        self.history_page_filtered(offset, limit, true)
    }

    fn history_page_filtered(
        &self,
        offset: u64,
        limit: u32,
        recycled_only: bool,
    ) -> Result<HistoryPage> {
        let filter = if recycled_only {
            " WHERE json_extract(data,'$.status')='recycled' AND time<=strftime('%s','now')"
        } else {
            ""
        };
        let offset = i64::try_from(offset).map_err(|_| anyhow!("历史分页位置超出范围"))?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let total = transaction.query_row(
            &format!("SELECT COUNT(*) FROM history{filter}"),
            [],
            |row| row.get(0),
        )?;
        let items = {
            let mut statement = transaction.prepare(&format!(
                "SELECT data FROM history{filter} ORDER BY time DESC,id {} LIMIT ?1 OFFSET ?2",
                if recycled_only { "DESC" } else { "ASC" }
            ))?;
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
