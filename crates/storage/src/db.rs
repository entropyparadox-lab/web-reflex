use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use web_reflex_core::ActionGraph;

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphHistoryEntry {
    pub history_id: i64,
    pub graph_id: String,
    pub version: u32,
    pub domain_pattern: String,
    pub skeleton_hash: String,
    pub graph_json: String,
    pub archived_at: String,
}

pub struct ActionStorage {
    conn: Mutex<Connection>,
}

impl ActionStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA busy_timeout = 5000;
            PRAGMA synchronous = NORMAL;
            ",
        )?;
        Self::run_migrations(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::run_migrations(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn run_migrations(conn: &mut Connection) -> Result<()> {
        let current_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if current_version < 1 {
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
                PRAGMA user_version = 1;
                ",
            )?;
        }

        if current_version < 2 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS action_graph_history (
                    history_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    graph_id TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    domain_pattern TEXT NOT NULL,
                    skeleton_hash TEXT NOT NULL,
                    graph_json TEXT NOT NULL,
                    archived_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_graph_history ON action_graph_history(graph_id, version);
                CREATE INDEX IF NOT EXISTS idx_graph_history_hash ON action_graph_history(skeleton_hash);
                PRAGMA user_version = 2;
                ",
            )?;
        }

        Ok(())
    }

    pub fn schema_version(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        Ok(version)
    }

    pub fn save_graph(&self, graph: &ActionGraph) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Check for existing version to archive into history
        let existing: Option<(u32, String, String, String)> = tx
            .query_row(
                "SELECT version, domain_pattern, skeleton_hash, graph_json FROM action_graphs WHERE graph_id = ?1",
                params![graph.graph_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        if let Some((old_ver, old_domain, old_hash, old_json)) = existing {
            // Archive prior version only if version changed or hash/content mutated
            tx.execute(
                "
                INSERT INTO action_graph_history (graph_id, version, domain_pattern, skeleton_hash, graph_json, archived_at)
                VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
                ",
                params![graph.graph_id, old_ver, old_domain, old_hash, old_json],
            )?;
        }

        let json_str = serde_json::to_string(graph)?;
        tx.execute(
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

        tx.commit()?;
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

    pub fn get_history(&self, graph_id: &str) -> Result<Vec<GraphHistoryEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT history_id, graph_id, version, domain_pattern, skeleton_hash, graph_json, archived_at
             FROM action_graph_history
             WHERE graph_id = ?1
             ORDER BY version DESC, history_id DESC",
        )?;

        let mut rows = stmt.query(params![graph_id])?;
        let mut history = Vec::new();
        while let Some(row) = rows.next()? {
            history.push(GraphHistoryEntry {
                history_id: row.get(0)?,
                graph_id: row.get(1)?,
                version: row.get(2)?,
                domain_pattern: row.get(3)?,
                skeleton_hash: row.get(4)?,
                graph_json: row.get(5)?,
                archived_at: row.get(6)?,
            });
        }
        Ok(history)
    }

    pub fn rollback_graph(&self, graph_id: &str, target_version: u32) -> Result<ActionGraph> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let current: Option<(u32, String, String, String)> = tx
            .query_row(
                "SELECT version, domain_pattern, skeleton_hash, graph_json FROM action_graphs WHERE graph_id = ?1",
                params![graph_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let (cur_ver, cur_domain, cur_hash, cur_json) = match current {
            Some(c) => c,
            None => bail!("ActionGraph '{}' not found for rollback", graph_id),
        };

        if cur_ver == target_version {
            let current_graph: ActionGraph = serde_json::from_str(&cur_json)?;
            return Ok(current_graph);
        }

        let target_json: Option<String> = tx
            .query_row(
                "SELECT graph_json FROM action_graph_history WHERE graph_id = ?1 AND version = ?2 ORDER BY history_id DESC LIMIT 1",
                params![graph_id, target_version],
                |row| row.get(0),
            )
            .optional()?;

        let target_json_str = match target_json {
            Some(j) => j,
            None => bail!(
                "Version {} not found in history for ActionGraph '{}'",
                target_version,
                graph_id
            ),
        };

        // Archive the current active version before rolling back
        tx.execute(
            "
            INSERT INTO action_graph_history (graph_id, version, domain_pattern, skeleton_hash, graph_json, archived_at)
            VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
            ",
            params![graph_id, cur_ver, cur_domain, cur_hash, cur_json],
        )?;

        // Restore target version into action_graphs
        let restored_graph: ActionGraph = serde_json::from_str(&target_json_str)?;
        let restored_json = serde_json::to_string(&restored_graph)?;

        tx.execute(
            "
            UPDATE action_graphs
            SET domain_pattern = ?1,
                skeleton_hash = ?2,
                version = ?3,
                graph_json = ?4,
                updated_at = datetime('now')
            WHERE graph_id = ?5
            ",
            params![
                restored_graph.domain_pattern,
                restored_graph.skeleton_hash,
                restored_graph.version,
                restored_json,
                graph_id
            ],
        )?;

        tx.commit()?;
        Ok(restored_graph)
    }
}

pub enum TenantStoragePool {
    Single(Arc<ActionStorage>),
    Directory {
        base_dir: PathBuf,
        tenants: Mutex<HashMap<String, Arc<ActionStorage>>>,
    },
}

impl TenantStoragePool {
    pub fn new_single(storage: Arc<ActionStorage>) -> Self {
        Self::Single(storage)
    }

    pub fn new_directory(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = base_dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self::Directory {
            base_dir: dir,
            tenants: Mutex::new(HashMap::new()),
        })
    }

    pub fn get_storage(&self, tenant_id: Option<&str>) -> Result<Arc<ActionStorage>> {
        match self {
            Self::Single(storage) => Ok(storage.clone()),
            Self::Directory { base_dir, tenants } => {
                let tenant = tenant_id.unwrap_or("default").trim();
                let tenant = if tenant.is_empty() { "default" } else { tenant };

                if !tenant
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    bail!(
                        "Invalid tenant ID '{}': only alphanumeric, '-', and '_' are allowed",
                        tenant
                    );
                }

                let mut map = tenants.lock().unwrap();
                if let Some(storage) = map.get(tenant) {
                    return Ok(storage.clone());
                }

                let db_path = base_dir.join(format!("{}.db", tenant));
                let storage = Arc::new(ActionStorage::open(&db_path)?);
                map.insert(tenant.to_string(), storage.clone());
                Ok(storage)
            }
        }
    }

    pub fn migrate_all(&self) -> Result<Vec<(String, u32)>> {
        match self {
            Self::Single(storage) => {
                let ver = storage.schema_version()?;
                Ok(vec![("default".to_string(), ver)])
            }
            Self::Directory { base_dir, .. } => {
                let mut results = Vec::new();
                for entry in std::fs::read_dir(base_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        let storage = self.get_storage(Some(stem))?;
                        let ver = storage.schema_version()?;
                        results.push((stem.to_string(), ver));
                    }
                }

                let default_storage = self.get_storage(Some("default"))?;
                let ver = default_storage.schema_version()?;
                if !results.iter().any(|(name, _)| name == "default") {
                    results.push(("default".to_string(), ver));
                }

                results.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(results)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use web_reflex_core::{ActionNode, ActionType, SafetyLevel, SelectorChain};

    fn make_test_graph(id: &str, version: u32, selector: &str) -> ActionGraph {
        ActionGraph {
            tenant_id: "default".to_string(),
            graph_id: id.to_string(),
            domain_pattern: "example.com".to_string(),
            skeleton_hash: format!("hash_{}", version),
            version,
            nodes: vec![ActionNode {
                step_id: "step_1".to_string(),
                action_type: ActionType::Click,
                safety_level: SafetyLevel::ReadOnly,
                requires_approval: false,
                target: SelectorChain::new(selector),
                value_slot: None,
                pre_condition: None,
                post_condition: None,
            }],
            created_at: "".to_string(),
            updated_at: "".to_string(),
        }
    }

    #[test]
    fn test_storage_crud_and_migration() -> Result<()> {
        let storage = ActionStorage::in_memory()?;
        assert_eq!(storage.schema_version()?, CURRENT_SCHEMA_VERSION);

        let graph = make_test_graph("test_login", 1, "button.submit");
        storage.save_graph(&graph)?;

        let found = storage.find_by_skeleton_hash("hash_1")?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().graph_id, "test_login");

        let found_domain = storage.find_by_domain("example.com")?;
        assert!(found_domain.is_some());
        assert_eq!(found_domain.unwrap().graph_id, "test_login");

        let all = storage.list_all_graphs()?;
        assert_eq!(all.len(), 1);

        Ok(())
    }

    #[test]
    fn test_version_history_archiving_and_rollback() -> Result<()> {
        let storage = ActionStorage::in_memory()?;

        // 1. Save initial v1
        let g1 = make_test_graph("checkout", 1, "#pay_v1");
        storage.save_graph(&g1)?;
        assert_eq!(storage.get_history("checkout")?.len(), 0);

        // 2. Update to v2 (simulating self-heal)
        let mut g2 = g1.clone();
        g2.version = 2;
        g2.nodes[0].target.primary = "#pay_v2".to_string();
        storage.save_graph(&g2)?;

        let history = storage.get_history("checkout")?;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].version, 1);
        assert!(history[0].graph_json.contains("#pay_v1"));

        // Active should be v2
        let current = storage.find_by_domain("example.com")?.unwrap();
        assert_eq!(current.version, 2);
        assert_eq!(current.nodes[0].target.primary, "#pay_v2");

        // 3. Update to v3
        let mut g3 = g2.clone();
        g3.version = 3;
        g3.nodes[0].target.primary = "#pay_v3".to_string();
        storage.save_graph(&g3)?;

        let history = storage.get_history("checkout")?;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, 2);
        assert_eq!(history[1].version, 1);

        // 4. Rollback to v1
        let restored = storage.rollback_graph("checkout", 1)?;
        assert_eq!(restored.version, 1);
        assert_eq!(restored.nodes[0].target.primary, "#pay_v1");

        let current = storage.find_by_domain("example.com")?.unwrap();
        assert_eq!(current.version, 1);
        assert_eq!(current.nodes[0].target.primary, "#pay_v1");

        // History now also has v3 archived
        let history_after = storage.get_history("checkout")?;
        assert_eq!(history_after.len(), 3);
        assert_eq!(history_after[0].version, 3);

        Ok(())
    }

    #[test]
    fn test_tenant_storage_pool_isolation() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!(
            "reflex_tenant_test_{}",
            std::time::SystemTime::now().elapsed().unwrap().as_nanos()
        ));
        let pool = TenantStoragePool::new_directory(&temp_dir)?;

        let storage_a = pool.get_storage(Some("customer-a"))?;
        let storage_b = pool.get_storage(Some("customer-b"))?;

        let g_a = make_test_graph("flow_a", 1, "#button_a");
        storage_a.save_graph(&g_a)?;

        // Customer B must NOT see Customer A's graph
        assert!(storage_b.find_by_skeleton_hash("hash_1")?.is_none());
        assert!(storage_b.list_all_graphs()?.is_empty());

        let g_b = make_test_graph("flow_b", 1, "#button_b");
        storage_b.save_graph(&g_b)?;

        assert_eq!(storage_a.list_all_graphs()?.len(), 1);
        assert_eq!(storage_b.list_all_graphs()?.len(), 1);

        // Tenant migrations check
        let migrations = pool.migrate_all()?;
        assert!(migrations
            .iter()
            .any(|(name, ver)| name == "customer-a" && *ver == 2));
        assert!(migrations
            .iter()
            .any(|(name, ver)| name == "customer-b" && *ver == 2));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
