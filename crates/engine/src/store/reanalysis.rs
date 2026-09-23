use super::*;

impl Store {
    pub fn restart_analysis(&self, scan: &str) -> Result<()> {
        self.require_finished(scan)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction()?;
        tx.execute("UPDATE analyses SET data=json_set(data,'$.status','stale','$.trace.superseded',json('true'),'$.message','已请求重新分析，此结果仅供历史参考') WHERE scan_id=?1 AND COALESCE(json_extract(data,'$.trace.source'),'llm')<>'jev'", [scan])?;
        tx.execute(
            "DELETE FROM kv WHERE key=?1",
            [format!("llm-budget:{scan}")],
        )?;
        tx.commit()?;
        Ok(())
    }
}
