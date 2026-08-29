use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use web_reflex_core::ActionGraph;

pub struct ActionStorage {
    conn: Mutex<Connection>,
}

impl ActionStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA busy_timeout = 5000;
            PRAGMA synchronous = NORMAL;
            ",
        )?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS action_graphs (
                graph_id TEXT PRIMARY KEY,
                domain_pattern TEXT NOT NULL,
                skeleton_hash TEXT NOT NULL,
                version INTEGER NOT NULL,
                graph_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_action_graphs_hash ON action_graphs(skeleton_hash);
            CREATE INDEX IF NOT EXISTS idx_action_graphs_domain ON action_graphs(domain_pattern);
            ",
        )?;
        Ok(())
    }

    pub fn save_graph(&self, graph: &ActionGraph) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json_str = serde_json::to_string(graph)?;
        conn.execute(
            "
            INSERT INTO action_graphs (graph_id, domain_pattern, skeleton_hash, version, graph_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))
            ON CONFLICT(graph_id) DO UPDATE SET
                domain_pattern = excluded.domain_pattern,
                skeleton_hash = excluded.skeleton_hash,
                version = excluded.version,
                graph_json = excluded.graph_json,
                updated_at = datetime('now');
            ",
            params![
                graph.graph_id,
                graph.domain_pattern,
                graph.skeleton_hash,
                graph.version,
                json_str
            ],
        )?;
        Ok(())
    }

    pub fn find_by_skeleton_hash(&self, hash: &str) -> Result<Option<ActionGraph>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT graph_json FROM action_graphs WHERE skeleton_hash = ?1 ORDER BY version DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![hash])?;
        if let Some(row) = rows.next()? {
            let json_str: String = row.get(0)?;
            let graph: ActionGraph = serde_json::from_str(&json_str)?;
            Ok(Some(graph))
        } else {
            Ok(None)
        }
    }

    pub fn find_by_domain(&self, domain: &str) -> Result<Option<ActionGraph>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT graph_json FROM action_graphs WHERE domain_pattern = ?1 ORDER BY version DESC LIMIT 1",
        )?;

        let mut rows = stmt.query(params![domain])?;
        if let Some(row) = rows.next()? {
            let json_str: String = row.get(0)?;
            let graph: ActionGraph = serde_json::from_str(&json_str)?;
            Ok(Some(graph))
        } else {
            Ok(None)
        }
    }

    pub fn list_all_graphs(&self) -> Result<Vec<ActionGraph>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT graph_json FROM action_graphs ORDER BY domain_pattern ASC, graph_id ASC",
        )?;

        let mut rows = stmt.query([])?;
        let mut graphs = Vec::new();
        while let Some(row) = rows.next()? {
            let json_str: String = row.get(0)?;
            let graph: ActionGraph = serde_json::from_str(&json_str)?;
            graphs.push(graph);
        }
        Ok(graphs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use web_reflex_core::{ActionNode, ActionType, SafetyLevel, SelectorChain};

    #[test]
    fn test_storage_crud() -> Result<()> {
        let storage = ActionStorage::in_memory()?;
        let graph = ActionGraph {
            graph_id: "test_login".to_string(),
            domain_pattern: "example.com".to_string(),
            skeleton_hash: "hash_12345".to_string(),
            version: 1,
            nodes: vec![ActionNode {
                step_id: "step_1".to_string(),
                action_type: ActionType::Click,
                safety_level: SafetyLevel::ReadOnly,
                requires_approval: false,
                target: SelectorChain::new("button.submit"),
                value_slot: None,
                pre_condition: None,
                post_condition: None,
            }],
            created_at: "".to_string(),
            updated_at: "".to_string(),
        };

        storage.save_graph(&graph)?;

        let found = storage.find_by_skeleton_hash("hash_12345")?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().graph_id, "test_login");

        let found_domain = storage.find_by_domain("example.com")?;
        assert!(found_domain.is_some());
        assert_eq!(found_domain.unwrap().graph_id, "test_login");

        let all = storage.list_all_graphs()?;
        assert_eq!(all.len(), 1);

        let not_found = storage.find_by_skeleton_hash("unknown_hash")?;
        assert!(not_found.is_none());

        Ok(())
    }
}
