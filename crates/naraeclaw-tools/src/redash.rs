//! Redash tool — query execution, dashboard lookup, ad-hoc SQL.
//!
//! Supported actions:
//! - `run_query`      — execute a saved query by ID or name
//! - `run_sql`        — execute arbitrary SQL against a data source
//! - `get_dashboard`  — fetch a dashboard's widget data as text
//! - `list_queries`   — search saved queries
//! - `list_data_sources` — list available data sources

use async_trait::async_trait;
use naraeclaw_api::tool::{Tool, ToolResult};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;

const POLL_INTERVAL_MS: u64 = 800;
const JOB_STATUS_SUCCESS: u64 = 3;
const JOB_STATUS_FAILURE: u64 = 4;
const JOB_STATUS_CANCELLED: u64 = 5;

pub struct RedashTool {
    base_url: String,
    api_key: String,
    max_rows: usize,
    timeout: Duration,
}

impl RedashTool {
    pub fn new(base_url: String, api_key: String, max_rows: usize, timeout_secs: u64) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            base_url,
            api_key,
            max_rows,
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    fn client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(self.timeout)
            .connect_timeout(Duration::from_secs(10))
            .user_agent("naraeclaw-redash/1.0")
            .build()
            .expect("reqwest client build failed")
    }

    fn auth(&self) -> (&'static str, String) {
        ("Authorization", format!("Key {}", self.api_key))
    }

    async fn get_json(&self, path: &str) -> anyhow::Result<Value> {
        let (header, value) = self.auth();
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client().get(&url).header(header, value).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Redash GET {path} returned {}", resp.status());
        }
        Ok(resp.json::<Value>().await?)
    }

    async fn post_json(&self, path: &str, body: &Value) -> anyhow::Result<Value> {
        let (header, value) = self.auth();
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client()
            .post(&url)
            .header(header, value)
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Redash POST {path} returned {status}: {text}");
        }
        Ok(resp.json::<Value>().await?)
    }

    /// Poll a job until it completes or times out.
    async fn poll_job(&self, job_id: &str) -> anyhow::Result<Value> {
        let deadline = tokio::time::Instant::now() + self.timeout;
        loop {
            if tokio::time::Instant::now() > deadline {
                anyhow::bail!("Redash job {job_id} timed out");
            }
            sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;

            let resp = self.get_json(&format!("/api/jobs/{job_id}")).await?;
            let status = resp["job"]["status"].as_u64().unwrap_or(0);
            match status {
                JOB_STATUS_SUCCESS => {
                    let result_id = resp["job"]["query_result_id"]
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("Job succeeded but no query_result_id"))?;
                    return self
                        .get_json(&format!("/api/query_results/{result_id}"))
                        .await;
                }
                JOB_STATUS_FAILURE => {
                    let error = resp["job"]["error"].as_str().unwrap_or("unknown error");
                    anyhow::bail!("Redash query failed: {error}");
                }
                JOB_STATUS_CANCELLED => anyhow::bail!("Redash job was cancelled"),
                _ => {}
            }
        }
    }

    /// Format query result rows as a markdown table (capped at max_rows).
    fn format_result(&self, query_result: &Value) -> String {
        let data = &query_result["query_result"]["data"];
        let columns: Vec<&str> = data["columns"]
            .as_array()
            .map(|cols| {
                cols.iter()
                    .filter_map(|c| c["name"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        let rows = data["rows"].as_array().map(|r| r.as_slice()).unwrap_or(&[]);

        if columns.is_empty() {
            return "No columns returned.".into();
        }

        let total = rows.len();
        let display_rows = &rows[..rows.len().min(self.max_rows)];

        // Markdown table header
        let header = format!("| {} |", columns.join(" | "));
        let sep = format!("| {} |", columns.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));

        let mut lines = vec![header, sep];
        for row in display_rows {
            let cells: Vec<String> = columns
                .iter()
                .map(|col| {
                    row.get(*col)
                        .map(|v| match v {
                            Value::Null => "null".into(),
                            Value::String(s) => s.replace('\n', " "),
                            other => other.to_string(),
                        })
                        .unwrap_or_default()
                })
                .collect();
            lines.push(format!("| {} |", cells.join(" | ")));
        }

        let mut out = lines.join("\n");
        if total > self.max_rows {
            out.push_str(&format!(
                "\n\n_(showing {} of {} rows — increase `max_rows` in config to see more)_",
                self.max_rows, total
            ));
        } else {
            out.push_str(&format!("\n\n_{total} row(s) returned._"));
        }
        out
    }

    // ── action handlers ──────────────────────────────────────────────

    async fn run_query(&self, args: &Value) -> anyhow::Result<String> {
        let query_id = if let Some(id) = args["query_id"].as_u64() {
            id
        } else if let Some(name) = args["query_name"].as_str() {
            self.find_query_id_by_name(name).await?
        } else {
            anyhow::bail!("run_query requires 'query_id' (integer) or 'query_name' (string)");
        };

        let params = args.get("parameters").cloned().unwrap_or(json!({}));
        let body = json!({ "parameters": params });

        let resp = self
            .post_json(&format!("/api/queries/{query_id}/results"), &body)
            .await?;

        let qr = if resp["query_result"].is_object() {
            resp
        } else if let Some(job_id) = resp["job"]["id"].as_str() {
            self.poll_job(job_id).await?
        } else {
            anyhow::bail!("Unexpected response from Redash: {resp}");
        };

        Ok(self.format_result(&qr))
    }

    async fn run_sql(&self, args: &Value) -> anyhow::Result<String> {
        let sql = args["sql"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("run_sql requires 'sql' parameter"))?;
        let data_source_id = args["data_source_id"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("run_sql requires 'data_source_id' (integer)"))?;

        let body = json!({
            "data_source_id": data_source_id,
            "query": sql,
            "max_age": 0
        });

        let resp = self.post_json("/api/query_results", &body).await?;

        let qr = if resp["query_result"].is_object() {
            resp
        } else if let Some(job_id) = resp["job"]["id"].as_str() {
            self.poll_job(job_id).await?
        } else {
            anyhow::bail!("Unexpected response from Redash: {resp}");
        };

        Ok(self.format_result(&qr))
    }

    async fn get_dashboard(&self, args: &Value) -> anyhow::Result<String> {
        let slug = args["dashboard_slug"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("get_dashboard requires 'dashboard_slug'"))?;

        let resp = self.get_json(&format!("/api/dashboards/{slug}")).await?;

        let name = resp["name"].as_str().unwrap_or(slug);
        let widgets = resp["widgets"].as_array().cloned().unwrap_or_default();

        let mut lines = vec![format!("# Dashboard: {name}\n")];
        for widget in &widgets {
            if let Some(vis) = widget["visualization"].as_object() {
                let title = vis
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(untitled)");
                let query_name = vis
                    .get("query")
                    .and_then(|q| q["name"].as_str())
                    .unwrap_or("?");
                lines.push(format!("- **{title}** (query: _{query_name}_)"));
            } else if let Some(text) = widget["text"].as_str() {
                if !text.trim().is_empty() {
                    lines.push(format!("  > {text}"));
                }
            }
        }

        if widgets.is_empty() {
            lines.push("_(no widgets found)_".into());
        } else {
            lines.push(format!("\n_{} widget(s). Use `run_query` with the query name/ID to fetch live data._", widgets.len()));
        }

        Ok(lines.join("\n"))
    }

    async fn list_queries(&self, args: &Value) -> anyhow::Result<String> {
        let search = args["search"].as_str().unwrap_or("");
        let path = if search.is_empty() {
            "/api/queries?page_size=50".to_string()
        } else {
            format!("/api/queries?q={}&page_size=50", urlencoding::encode(search))
        };

        let resp = self.get_json(&path).await?;
        let results = resp["results"].as_array().cloned().unwrap_or_default();

        if results.is_empty() {
            return Ok("No queries found.".into());
        }

        let mut lines = vec!["| ID | Name | Data Source | Updated |".into(), "| --- | --- | --- | --- |".into()];
        for q in results.iter().take(50) {
            let id = q["id"].as_u64().unwrap_or(0);
            let name = q["name"].as_str().unwrap_or("?");
            let ds = q["data_source_id"].as_u64().unwrap_or(0);
            let updated = q["updated_at"].as_str().unwrap_or("?");
            lines.push(format!("| {id} | {name} | {ds} | {updated} |"));
        }

        Ok(lines.join("\n"))
    }

    async fn list_data_sources(&self) -> anyhow::Result<String> {
        let resp = self.get_json("/api/data_sources").await?;
        let sources = resp.as_array().cloned().unwrap_or_default();

        if sources.is_empty() {
            return Ok("No data sources found.".into());
        }

        let mut lines = vec!["| ID | Name | Type |".into(), "| --- | --- | --- |".into()];
        for ds in &sources {
            let id = ds["id"].as_u64().unwrap_or(0);
            let name = ds["name"].as_str().unwrap_or("?");
            let kind = ds["type"].as_str().unwrap_or("?");
            lines.push(format!("| {id} | {name} | {kind} |"));
        }

        Ok(lines.join("\n"))
    }

    async fn find_query_id_by_name(&self, name: &str) -> anyhow::Result<u64> {
        let path = format!("/api/queries?q={}&page_size=20", urlencoding::encode(name));
        let resp = self.get_json(&path).await?;
        let results = resp["results"].as_array().cloned().unwrap_or_default();

        // Exact match first, then case-insensitive prefix
        let name_lower = name.to_lowercase();
        results
            .iter()
            .find(|q| q["name"].as_str().unwrap_or("") == name)
            .or_else(|| {
                results.iter().find(|q| {
                    q["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .starts_with(&name_lower)
                })
            })
            .and_then(|q| q["id"].as_u64())
            .ok_or_else(|| anyhow::anyhow!("No query found with name '{name}'"))
    }
}

// ── Tool trait ────────────────────────────────────────────────────────────────

#[async_trait]
impl Tool for RedashTool {
    fn name(&self) -> &str {
        "redash"
    }

    fn description(&self) -> &str {
        "Query and explore data via Redash. \
         Run saved queries by ID or name, execute ad-hoc SQL against any connected data source, \
         browse dashboards, and list available queries. \
         Use 'list_data_sources' first to find data_source_id values for ad-hoc SQL. \
         Use 'list_queries' to discover saved queries. \
         Results are returned as markdown tables (capped by max_rows in config)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["run_query", "run_sql", "get_dashboard", "list_queries", "list_data_sources"],
                    "description": "Action to perform."
                },
                "query_id": {
                    "type": "integer",
                    "description": "Saved query ID (for run_query)."
                },
                "query_name": {
                    "type": "string",
                    "description": "Saved query name — searches for an exact or prefix match (for run_query)."
                },
                "sql": {
                    "type": "string",
                    "description": "SQL statement to execute (for run_sql)."
                },
                "data_source_id": {
                    "type": "integer",
                    "description": "Data source ID to run the SQL against (for run_sql). Use list_data_sources to discover IDs."
                },
                "parameters": {
                    "type": "object",
                    "description": "Query parameters as key-value pairs (for run_query with parameterised queries)."
                },
                "dashboard_slug": {
                    "type": "string",
                    "description": "Dashboard URL slug (for get_dashboard)."
                },
                "search": {
                    "type": "string",
                    "description": "Optional search string to filter queries (for list_queries)."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let action = match args["action"].as_str() {
            Some(a) => a,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("Missing required parameter 'action'".into()),
                });
            }
        };

        let result = match action {
            "run_query" => self.run_query(&args).await,
            "run_sql" => self.run_sql(&args).await,
            "get_dashboard" => self.get_dashboard(&args).await,
            "list_queries" => self.list_queries(&args).await,
            "list_data_sources" => self.list_data_sources().await,
            other => Err(anyhow::anyhow!("Unknown action '{other}'")),
        };

        match result {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> RedashTool {
        RedashTool::new("https://redash.example.com".into(), "testkey".into(), 200, 60)
    }

    #[test]
    fn name_is_redash() {
        assert_eq!(tool().name(), "redash");
    }

    #[test]
    fn description_is_non_empty() {
        assert!(!tool().description().is_empty());
    }

    #[test]
    fn schema_requires_action() {
        let schema = tool().parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&Value::String("action".into())));
    }

    #[test]
    fn schema_action_has_enum() {
        let schema = tool().parameters_schema();
        let enums = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert!(enums.contains(&Value::String("run_query".into())));
        assert!(enums.contains(&Value::String("run_sql".into())));
        assert!(enums.contains(&Value::String("list_data_sources".into())));
    }

    #[tokio::test]
    async fn execute_missing_action_returns_error() {
        let result = tool().execute(json!({})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("action"));
    }

    #[tokio::test]
    async fn execute_unknown_action_returns_error() {
        let result = tool().execute(json!({"action": "fly"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("fly"));
    }

    #[test]
    fn format_result_basic_table() {
        let t = tool();
        let qr = json!({
            "query_result": {
                "data": {
                    "columns": [{"name": "id"}, {"name": "name"}],
                    "rows": [
                        {"id": 1, "name": "Alice"},
                        {"id": 2, "name": "Bob"}
                    ]
                }
            }
        });
        let out = t.format_result(&qr);
        assert!(out.contains("| id | name |"));
        assert!(out.contains("Alice"));
        assert!(out.contains("Bob"));
        assert!(out.contains("2 row(s)"));
    }

    #[test]
    fn format_result_respects_max_rows() {
        let t = RedashTool::new("https://x.com".into(), "k".into(), 1, 30);
        let rows: Vec<Value> = (0..5).map(|i| json!({"n": i})).collect();
        let qr = json!({
            "query_result": {
                "data": {
                    "columns": [{"name": "n"}],
                    "rows": rows
                }
            }
        });
        let out = t.format_result(&qr);
        assert!(out.contains("showing 1 of 5"));
    }

    #[test]
    fn format_result_empty_columns() {
        let t = tool();
        let qr = json!({ "query_result": { "data": { "columns": [], "rows": [] } } });
        assert_eq!(t.format_result(&qr), "No columns returned.");
    }
}
