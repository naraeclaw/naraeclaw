use crate::config::Config;
use anyhow::{Context, Result, bail, ensure};
use naraeclaw_memory::effective_memory_backend_name;
use naraeclaw_memory::knowledge_graph::{KnowledgeEdge, KnowledgeGraph, KnowledgeNode, NodeType};
use naraeclaw_memory::snapshot::SNAPSHOT_FILENAME as LEGACY_MEMORY_SNAPSHOT_FILENAME;
use naraeclaw_tools::mcp_client::McpRegistry;
use rusqlite::{Connection, OpenFlags, OptionalExtension, backup::Backup};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const MANIFEST_VERSION: u32 = 1;
const MAX_BYORI_NAME_CHARS: usize = 256;
const MAX_BYORI_BODY_CHARS: usize = 65_536;
const MAX_BYORI_KIND_CHARS: usize = 64;
const LEGACY_QDRANT_MIGRATION_GUIDANCE: &str = "legacy Qdrant migration is unsupported: keep `[knowledge].enabled = false` so the legacy Qdrant backend remains active, export the Qdrant collection separately, and plan a separate ByoriDB import before enabling knowledge";
const SAFE_TOOL_NAMES: &[&str] = &[
    "byoridb__memory_remember",
    "byoridb__memory_recall",
    "byoridb__memory_query_read",
    "byoridb__memory_wiki_upsert",
    "byoridb__memory_link",
    "byoridb__memory_read",
    "byoridb__memory_export",
];
const MIGRATION_TOOL_NAMES: &[&str] = &[
    "byoridb__memory_remember",
    "byoridb__memory_wiki_upsert",
    "byoridb__memory_link",
    "byoridb__memory_read",
];
const UNSAFE_TOOL_NAME: &str = "byoridb__memory_query";

#[derive(Debug, Clone)]
struct LegacySources {
    config: Option<PathBuf>,
    markdown: Vec<MarkdownSource>,
    memory_snapshot: Option<PathBuf>,
    memory_db: Option<PathBuf>,
    graph_dbs: Vec<LegacyGraphSource>,
}

#[derive(Debug, Clone)]
struct MarkdownSource {
    path: PathBuf,
    logical_name: String,
    category: String,
}

#[derive(Debug, Clone)]
struct LegacyGraphSource {
    path: PathBuf,
    logical_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct SnapshotFile {
    kind: String,
    source: String,
    snapshot: String,
}

#[derive(Debug, Clone)]
struct SnapshotSet {
    root: PathBuf,
    files: Vec<SnapshotFile>,
    markdown: Vec<MarkdownSource>,
    memory_snapshot: Option<PathBuf>,
    memory_db: Option<(PathBuf, PathBuf)>,
    graph_dbs: Vec<SnapshotGraphDb>,
}

#[derive(Debug, Clone)]
struct SnapshotGraphDb {
    logical_name: String,
    original: PathBuf,
    snapshot: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
struct MigrationCounts {
    markdown_notes: usize,
    memory_snapshot_notes: usize,
    sqlite_memory_notes: usize,
    graph_nodes: usize,
    graph_edges: usize,
}

impl MigrationCounts {
    fn total_items(&self) -> usize {
        self.markdown_notes
            + self.memory_snapshot_notes
            + self.sqlite_memory_notes
            + self.graph_nodes
    }
}

#[derive(Debug, Clone, Serialize)]
struct LegacyMemoryNote {
    canonical_name: String,
    kind: String,
    origin: String,
    source: String,
    legacy_id: String,
    key: String,
    content: String,
    category: String,
    raw_markdown: Option<String>,
    line: Option<usize>,
    created_at: Option<String>,
    updated_at: Option<String>,
    session_id: Option<String>,
    namespace: Option<String>,
    importance: Option<f64>,
    superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LegacyGraphExport {
    /// Stable identity used in durable Byori names and bodies.
    source: String,
    /// Original absolute/expanded path, retained only in the migration manifest.
    original_source: String,
    /// Snapshot path, retained only in the migration manifest.
    snapshot: String,
    nodes: Vec<KnowledgeNode>,
    edges: Vec<KnowledgeEdge>,
}

#[derive(Debug, Clone, Default, Serialize)]
struct LegacyInventory {
    notes: Vec<LegacyMemoryNote>,
    graphs: Vec<LegacyGraphExport>,
    counts: MigrationCounts,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ApplyStats {
    notes_upserted: usize,
    notes_unchanged: usize,
    graph_nodes_upserted: usize,
    graph_nodes_unchanged: usize,
    links_upserted: usize,
}

#[derive(Debug, Serialize)]
struct MigrationManifest {
    manifest_version: u32,
    migration_id: String,
    created_at: String,
    status: String,
    include_daily: bool,
    byori_space: String,
    sources: Vec<SnapshotFile>,
    inventory: LegacyInventory,
    apply: Option<ApplyStats>,
    error: Option<String>,
}

/// Handle `naraeclaw knowledge <subcommand>`.
pub async fn handle_command(command: crate::KnowledgeCommands, config: &Config) -> Result<()> {
    match command {
        crate::KnowledgeCommands::Status => handle_status(config).await,
        crate::KnowledgeCommands::Migrate {
            dry_run,
            include_daily,
            yes,
        } => handle_migrate(config, dry_run, include_daily, yes).await,
    }
}

async fn handle_status(config: &Config) -> Result<()> {
    let space = config.byori_space_name();
    let server = config.byori_mcp_server_config();
    let wrapper = PathBuf::from(&server.command);
    let legacy_backend = effective_legacy_memory_backend(config);
    let legacy_configured = legacy_backend != "none" || config.memory.auto_save;
    let migration_blocker = legacy_qdrant_migration_blocker(config);

    println!("ByoriDB Knowledge Status\n");
    println!("  Enabled:          {}", config.uses_byori_knowledge());
    println!("  Provider:         {}", config.knowledge.provider);
    println!("  Space:            {space}");
    println!("  Managed wrapper:  {}", wrapper.display());
    println!(
        "  Wrapper exists:   {}",
        if wrapper.is_file() { "yes" } else { "no" }
    );
    println!("  Required:         {}", config.knowledge.required);
    if config.uses_byori_knowledge() {
        println!(
            "  Legacy memory:    {}",
            if legacy_configured {
                format!("configured but inactive ({legacy_backend})")
            } else {
                "disabled".to_string()
            }
        );
    } else {
        println!("  Legacy memory:    active ({legacy_backend})");
    }
    if let Some(blocker) = migration_blocker {
        println!("  Migration:        BLOCKED — {blocker}");
    } else if config.uses_byori_knowledge() && legacy_configured {
        println!("  Migration:        run `naraeclaw knowledge migrate --dry-run`");
    }

    if !config.uses_byori_knowledge() {
        println!("  MCP connection:   disabled");
        return Ok(());
    }
    if !wrapper.is_file() {
        bail!(
            "managed ByoriDB wrapper is missing at {}; install ByoriDB or update knowledge.byoridb_home",
            wrapper.display()
        );
    }

    let registry = McpRegistry::connect_all(&[server]).await?;
    if registry.is_empty() {
        println!("  MCP connection:   failed");
        bail!("could not connect to the managed ByoriDB MCP server");
    }
    println!("  MCP connection:   ready");

    let tools: HashSet<String> = registry.tool_names().into_iter().collect();
    let missing = missing_tools(&tools, SAFE_TOOL_NAMES);
    let unsafe_visible = tools.contains(UNSAFE_TOOL_NAME);
    println!(
        "  Safe tools:       {}",
        if missing.is_empty() {
            "ready".to_string()
        } else {
            format!("missing {}", missing.join(", "))
        }
    );
    println!(
        "  Unsafe query:     {}",
        if unsafe_visible { "exposed" } else { "hidden" }
    );

    if !missing.is_empty() || unsafe_visible {
        bail!("ByoriDB MCP server does not expose the required safe profile");
    }

    let export = call_tool_json(
        &registry,
        "byoridb__memory_export",
        json!({"limit": 1, "offset": 0, "include_links": false}),
    )
    .await
    .context("ByoriDB export readiness probe failed")?;
    let export_ready = export.get("schema_version").is_some()
        && export.get("items").is_some_and(Value::is_array)
        && export.get("space").and_then(Value::as_str) == Some(space.as_str());
    println!(
        "  Export readiness: {}",
        if export_ready {
            "ready"
        } else {
            "invalid response"
        }
    );
    ensure!(
        export_ready,
        "ByoriDB export readiness probe returned an invalid payload"
    );

    Ok(())
}

async fn handle_migrate(
    config: &Config,
    dry_run: bool,
    include_daily: bool,
    yes: bool,
) -> Result<()> {
    ensure_supported_legacy_migration(config)?;

    let sources = discover_legacy_sources(config, include_daily)?;
    let source_counts = count_legacy_sources(&sources, include_daily)?;
    print_legacy_sources(&sources);
    print_migration_counts(&source_counts, include_daily, dry_run);

    if dry_run {
        println!("\nDry run complete. No source files or configuration were modified.");
        return Ok(());
    }
    if !yes {
        bail!("knowledge migrate requires --yes for an actual migration; use --dry-run to preview");
    }
    if source_counts.total_items() == 0 && source_counts.graph_edges == 0 {
        println!("\nNo eligible legacy knowledge was found. Nothing was changed.");
        return Ok(());
    }

    let snapshot = create_snapshot(config, &sources)?;
    println!("\nSnapshot: {}", snapshot.root.display());
    let migration_id = snapshot
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("byori-migration")
        .to_string();
    let mut manifest = MigrationManifest {
        manifest_version: MANIFEST_VERSION,
        migration_id,
        created_at: chrono::Utc::now().to_rfc3339(),
        status: "snapshotted".to_string(),
        include_daily,
        byori_space: config.byori_space_name(),
        sources: snapshot.files.clone(),
        inventory: LegacyInventory::default(),
        apply: None,
        error: None,
    };
    write_manifest(&snapshot.root, &manifest)?;

    let inventory = match load_snapshot_inventory(&snapshot, include_daily) {
        Ok(inventory) => inventory,
        Err(error) => {
            manifest.status = "failed".to_string();
            manifest.error = Some(format!("{error:#}"));
            let _ = write_manifest(&snapshot.root, &manifest);
            return Err(error).with_context(|| {
                format!(
                    "failed to read migration snapshot; source files are unchanged and the snapshot remains at {}",
                    snapshot.root.display()
                )
            });
        }
    };
    manifest.inventory = inventory.clone();
    write_manifest(&snapshot.root, &manifest)?;

    match apply_inventory(config, &inventory).await {
        Ok(stats) => {
            manifest.status = "complete".to_string();
            manifest.apply = Some(stats.clone());
            write_manifest(&snapshot.root, &manifest)?;
            println!(
                "Migration complete: {} notes upserted, {} notes unchanged, {} graph nodes upserted, {} graph nodes unchanged, {} links upserted.",
                stats.notes_upserted,
                stats.notes_unchanged,
                stats.graph_nodes_upserted,
                stats.graph_nodes_unchanged,
                stats.links_upserted,
            );
            println!("Legacy sources were retained and configuration was not rewritten.");
            Ok(())
        }
        Err(error) => {
            manifest.status = "failed".to_string();
            manifest.error = Some(format!("{error:#}"));
            let _ = write_manifest(&snapshot.root, &manifest);
            Err(error).with_context(|| {
                format!(
                    "migration failed; source files are unchanged and the auditable snapshot remains at {}",
                    snapshot.root.display()
                )
            })
        }
    }
}

fn effective_legacy_memory_backend(config: &Config) -> String {
    effective_memory_backend_name(
        &config.memory.backend,
        Some(&config.storage.provider.config),
    )
}

fn legacy_qdrant_migration_blocker(config: &Config) -> Option<&'static str> {
    effective_legacy_memory_backend(config)
        .eq_ignore_ascii_case("qdrant")
        .then_some(LEGACY_QDRANT_MIGRATION_GUIDANCE)
}

fn ensure_supported_legacy_migration(config: &Config) -> Result<()> {
    if let Some(blocker) = legacy_qdrant_migration_blocker(config) {
        bail!(blocker);
    }
    Ok(())
}

fn print_legacy_sources(sources: &LegacySources) {
    println!("Legacy sources:");
    if let Some(config) = &sources.config {
        println!("  config:             {}", config.display());
    }
    for markdown in &sources.markdown {
        println!(
            "  markdown ({:<5}):  {}",
            markdown.category,
            markdown.path.display()
        );
    }
    if let Some(snapshot) = &sources.memory_snapshot {
        println!("  memory snapshot:    {}", snapshot.display());
    }
    if let Some(database) = &sources.memory_db {
        println!("  sqlite memory:      {}", database.display());
    }
    for graph in &sources.graph_dbs {
        println!(
            "  graph ({:<20}): {}",
            graph.logical_name,
            graph.path.display()
        );
    }
    if sources.config.is_none()
        && sources.markdown.is_empty()
        && sources.memory_snapshot.is_none()
        && sources.memory_db.is_none()
        && sources.graph_dbs.is_empty()
    {
        println!("  (none)");
    }
    println!();
}

fn print_migration_counts(counts: &MigrationCounts, include_daily: bool, dry_run: bool) {
    println!(
        "Legacy knowledge migration{}\n",
        if dry_run { " (dry run)" } else { "" }
    );
    println!("  MEMORY.md/daily notes:  {}", counts.markdown_notes);
    println!("  MEMORY_SNAPSHOT notes:  {}", counts.memory_snapshot_notes);
    println!("  brain.db memories:      {}", counts.sqlite_memory_notes);
    println!("  knowledge graph nodes:  {}", counts.graph_nodes);
    println!("  knowledge graph edges:  {}", counts.graph_edges);
    println!("  Daily included:         {include_daily}");
    println!("  Conversation included:  false");
}

fn missing_tools(tools: &HashSet<String>, required: &[&str]) -> Vec<String> {
    required
        .iter()
        .filter(|tool| !tools.contains(**tool))
        .map(|tool| (*tool).to_string())
        .collect()
}

async fn call_tool_json(registry: &McpRegistry, tool: &str, arguments: Value) -> Result<Value> {
    let raw = registry.call_tool(tool, arguments).await?;
    parse_mcp_tool_payload(&raw)
        .with_context(|| format!("failed to decode MCP tool response from {tool}"))
}

fn parse_mcp_tool_payload(raw: &str) -> Result<Value> {
    let envelope: Value = serde_json::from_str(raw).context("MCP result is not JSON")?;
    let text = envelope
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| {
            content
                .iter()
                .find_map(|item| item.get("text").and_then(Value::as_str))
        })
        .context("MCP result contains no text content")?;
    ensure!(
        envelope.get("isError").and_then(Value::as_bool) != Some(true),
        "MCP tool returned an error: {text}"
    );
    serde_json::from_str(text).context("MCP text content is not JSON")
}

fn discover_legacy_sources(config: &Config, include_daily: bool) -> Result<LegacySources> {
    let config_path = config
        .config_path
        .is_file()
        .then(|| config.config_path.clone());
    let mut markdown = Vec::new();
    let memory_md = config.workspace_dir.join("MEMORY.md");
    if memory_md.is_file() {
        markdown.push(MarkdownSource {
            path: memory_md,
            logical_name: "MEMORY.md".to_string(),
            category: "core".to_string(),
        });
    }
    if include_daily {
        let memory_dir = config.workspace_dir.join("memory");
        if memory_dir.is_dir() {
            let mut daily_paths = fs::read_dir(&memory_dir)
                .with_context(|| format!("failed to read {}", memory_dir.display()))?
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
                .collect::<Vec<_>>();
            daily_paths.sort();
            for path in daily_paths {
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("daily.md");
                markdown.push(MarkdownSource {
                    logical_name: format!("memory/{file_name}"),
                    path,
                    category: "daily".to_string(),
                });
            }
        }
    }

    let memory_db_path = config.workspace_dir.join("memory").join("brain.db");
    let memory_db = memory_db_path.is_file().then_some(memory_db_path);
    let memory_snapshot_path = config.workspace_dir.join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
    // This file is a recovery mirror of brain.db, matching the legacy auto-hydration
    // contract. Treating both as independent sources would duplicate core memories.
    let memory_snapshot =
        (memory_db.is_none() && memory_snapshot_path.is_file()).then_some(memory_snapshot_path);

    let mut graph_dbs = Vec::new();
    if let Some(home) = directories::UserDirs::new() {
        push_unique_graph_source(
            &mut graph_dbs,
            home.home_dir().join(".naraeclaw").join("knowledge.db"),
            "knowledge/default",
        );
    }
    if let Some(path) = legacy_graph_path_from_config(config)? {
        push_unique_graph_source(&mut graph_dbs, path, "knowledge/configured");
    }

    Ok(LegacySources {
        config: config_path,
        markdown,
        memory_snapshot,
        memory_db,
        graph_dbs,
    })
}

fn legacy_graph_path_from_config(config: &Config) -> Result<Option<PathBuf>> {
    if !config.config_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&config.config_path)
        .with_context(|| format!("failed to read {}", config.config_path.display()))?;
    let Some(raw_path) = legacy_db_path_from_toml(&raw)? else {
        return Ok(None);
    };
    let expanded = PathBuf::from(shellexpand::tilde(&raw_path).as_ref());
    let resolved = if expanded.is_absolute() {
        expanded
    } else {
        config
            .config_path
            .parent()
            .unwrap_or(&config.workspace_dir)
            .join(expanded)
    };
    Ok(Some(resolved))
}

fn legacy_db_path_from_toml(raw: &str) -> Result<Option<String>> {
    let parsed: toml::Value = toml::from_str(raw).context("failed to parse legacy config TOML")?;
    Ok(parsed
        .get("knowledge")
        .and_then(|knowledge| knowledge.get("db_path"))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToString::to_string))
}

fn push_unique_graph_source(
    sources: &mut Vec<LegacyGraphSource>,
    candidate: PathBuf,
    logical_name: &str,
) {
    if !candidate.is_file() {
        return;
    }
    let identity = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    if sources.iter().any(|existing| {
        existing
            .path
            .canonicalize()
            .unwrap_or_else(|_| existing.path.clone())
            == identity
    }) {
        return;
    }
    sources.push(LegacyGraphSource {
        path: candidate,
        logical_name: logical_name.to_string(),
    });
}

fn count_legacy_sources(sources: &LegacySources, include_daily: bool) -> Result<MigrationCounts> {
    let markdown_notes = sources.markdown.iter().try_fold(0usize, |count, source| {
        parse_markdown_file(source).map(|notes| count + notes.len())
    })?;
    let memory_snapshot_notes = sources.memory_snapshot.as_ref().map_or(Ok(0), |path| {
        read_legacy_memory_snapshot(path).map(|notes| notes.len())
    })?;
    let sqlite_memory_notes = sources.memory_db.as_ref().map_or(Ok(0), |path| {
        read_memory_database(path, "memory/brain.db", include_daily).map(|notes| notes.len())
    })?;
    let (graph_nodes, graph_edges) =
        sources
            .graph_dbs
            .iter()
            .try_fold((0usize, 0usize), |(nodes, edges), source| {
                let (next_nodes, next_edges) = count_graph_database(&source.path)?;
                Ok::<_, anyhow::Error>((nodes + next_nodes, edges + next_edges))
            })?;
    Ok(MigrationCounts {
        markdown_notes,
        memory_snapshot_notes,
        sqlite_memory_notes,
        graph_nodes,
        graph_edges,
    })
}

fn open_read_only(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open legacy SQLite database {}", path.display()))
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            [table],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .context("failed to inspect legacy SQLite schema")
}

fn table_columns(connection: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .context("failed to inspect legacy SQLite columns")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<HashSet<_>, _>>()?;
    Ok(columns)
}

fn count_graph_database(path: &Path) -> Result<(usize, usize)> {
    let connection = open_read_only(path)?;
    let nodes = if table_exists(&connection, "nodes")? {
        connection
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .with_context(|| format!("failed to count nodes in {}", path.display()))?
    } else {
        0
    };
    let edges = if table_exists(&connection, "edges")? {
        connection
            .query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))
            .with_context(|| format!("failed to count edges in {}", path.display()))?
    } else {
        0
    };
    Ok((nodes, edges))
}

fn parse_markdown_file(source: &MarkdownSource) -> Result<Vec<LegacyMemoryNote>> {
    let content = fs::read_to_string(&source.path)
        .with_context(|| format!("failed to read {}", source.path.display()))?;
    Ok(parse_markdown_content(source, &content))
}

fn parse_markdown_content(source: &MarkdownSource, content: &str) -> Vec<LegacyMemoryNote> {
    content
        .lines()
        .enumerate()
        .filter_map(|(line_index, raw_line)| {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }

            let clean = trimmed.strip_prefix("- ").unwrap_or(trimmed);
            let (key, body) = parse_markdown_entry(clean).unwrap_or_else(|| {
                (
                    format!("{}:{}", source.logical_name, line_index + 1),
                    clean.to_string(),
                )
            });
            let legacy_id = format!("{}:{}", source.logical_name, line_index + 1);
            let canonical_name = deterministic_note_name(&[
                "markdown",
                &source.logical_name,
                &source.category,
                &legacy_id,
                &key,
            ]);
            Some(LegacyMemoryNote {
                canonical_name,
                kind: source.category.clone(),
                origin: "markdown".to_string(),
                source: source.logical_name.clone(),
                legacy_id,
                key,
                content: body,
                category: source.category.clone(),
                raw_markdown: Some(raw_line.to_string()),
                line: Some(line_index + 1),
                created_at: None,
                updated_at: None,
                session_id: None,
                namespace: None,
                importance: None,
                superseded_by: None,
            })
        })
        .collect()
}

fn parse_markdown_entry(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("**")?;
    let marker = rest.find("**:")?;
    let key = rest[..marker].trim();
    if key.is_empty() {
        return None;
    }
    let content = rest[marker + 3..].trim_start();
    Some((key.to_string(), content.to_string()))
}

#[derive(Debug)]
struct PendingMemorySnapshotEntry {
    key: String,
    heading_line: usize,
    content_lines: Vec<String>,
    raw_lines: Vec<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

fn read_legacy_memory_snapshot(path: &Path) -> Result<Vec<LegacyMemoryNote>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read legacy snapshot {}", path.display()))?;
    Ok(parse_legacy_memory_snapshot(&content))
}

fn parse_legacy_memory_snapshot(content: &str) -> Vec<LegacyMemoryNote> {
    let mut notes = Vec::new();
    let mut pending: Option<PendingMemorySnapshotEntry> = None;

    for (line_index, raw_line) in content.lines().enumerate() {
        let trimmed = raw_line.trim();
        if let Some(key) = snapshot_heading_key(trimmed) {
            finish_memory_snapshot_entry(&mut notes, pending.take());
            pending = Some(PendingMemorySnapshotEntry {
                key,
                heading_line: line_index + 1,
                content_lines: Vec::new(),
                raw_lines: vec![raw_line.to_string()],
                created_at: None,
                updated_at: None,
            });
            continue;
        }

        let Some(entry) = pending.as_mut() else {
            continue;
        };
        entry.raw_lines.push(raw_line.to_string());
        if let Some((created_at, updated_at)) = snapshot_entry_metadata(trimmed) {
            entry.created_at = Some(created_at);
            entry.updated_at = Some(updated_at);
        } else if trimmed != "---" {
            entry.content_lines.push(raw_line.to_string());
        }
    }
    finish_memory_snapshot_entry(&mut notes, pending);
    notes
}

fn snapshot_heading_key(line: &str) -> Option<String> {
    line.strip_prefix("### 🔑 `")
        .and_then(|rest| rest.strip_suffix('`'))
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string)
}

fn snapshot_entry_metadata(line: &str) -> Option<(String, String)> {
    let metadata = line.strip_prefix("*Created: ")?.strip_suffix('*')?;
    let (created_at, updated_at) = metadata.split_once(" | Updated: ")?;
    Some((created_at.trim().to_string(), updated_at.trim().to_string()))
}

fn finish_memory_snapshot_entry(
    notes: &mut Vec<LegacyMemoryNote>,
    pending: Option<PendingMemorySnapshotEntry>,
) {
    let Some(entry) = pending else {
        return;
    };
    let content = entry.content_lines.join("\n").trim().to_string();
    if content.is_empty() {
        return;
    }
    let canonical_name = deterministic_note_name(&[
        "memory-snapshot",
        LEGACY_MEMORY_SNAPSHOT_FILENAME,
        "core",
        &entry.key,
    ]);
    notes.push(LegacyMemoryNote {
        canonical_name,
        kind: "core".to_string(),
        origin: "memory-snapshot".to_string(),
        source: LEGACY_MEMORY_SNAPSHOT_FILENAME.to_string(),
        legacy_id: entry.key.clone(),
        key: entry.key,
        content,
        category: "core".to_string(),
        raw_markdown: Some(entry.raw_lines.join("\n")),
        line: Some(entry.heading_line),
        created_at: entry.created_at,
        updated_at: entry.updated_at,
        session_id: None,
        namespace: None,
        importance: None,
        superseded_by: None,
    });
}

fn deterministic_note_name(parts: &[&str]) -> String {
    format!("legacy-memory:{}", stable_digest(parts))
}

fn stable_digest(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.len().to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())[..24].to_string()
}

fn category_is_eligible(category: &str, include_daily: bool) -> bool {
    let normalized = category.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "conversation" => false,
        "daily" => include_daily,
        _ => true,
    }
}

fn optional_column<'a>(columns: &HashSet<String>, name: &'a str) -> &'a str {
    if columns.contains(name) { name } else { "NULL" }
}

fn read_memory_database(
    path: &Path,
    logical_source: &str,
    include_daily: bool,
) -> Result<Vec<LegacyMemoryNote>> {
    let connection = open_read_only(path)?;
    if !table_exists(&connection, "memories")? {
        return Ok(Vec::new());
    }

    let columns = table_columns(&connection, "memories")?;
    for required in [
        "id",
        "key",
        "content",
        "category",
        "created_at",
        "updated_at",
    ] {
        ensure!(
            columns.contains(required),
            "legacy memory database {} is missing required column {required}",
            path.display()
        );
    }

    let sql = format!(
        "SELECT id, key, content, category, created_at, updated_at, \
         {}, {}, {}, {} \
         FROM memories ORDER BY category ASC, key ASC, id ASC",
        optional_column(&columns, "session_id"),
        optional_column(&columns, "namespace"),
        optional_column(&columns, "importance"),
        optional_column(&columns, "superseded_by"),
    );
    let mut statement = connection
        .prepare(&sql)
        .with_context(|| format!("failed to read legacy memories from {}", path.display()))?;
    let rows = statement.query_map([], |row| {
        let legacy_id: String = row.get(0)?;
        let key: String = row.get(1)?;
        let content: String = row.get(2)?;
        let category: String = row.get(3)?;
        Ok(LegacyMemoryNote {
            canonical_name: deterministic_note_name(&[
                "sqlite",
                logical_source,
                &category,
                &legacy_id,
                &key,
            ]),
            kind: category.clone(),
            origin: "sqlite".to_string(),
            source: logical_source.to_string(),
            legacy_id,
            key,
            content,
            category,
            raw_markdown: None,
            line: None,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            session_id: row.get(6)?,
            namespace: row.get(7)?,
            importance: row.get(8)?,
            superseded_by: row.get(9)?,
        })
    })?;

    let mut notes = Vec::new();
    for row in rows {
        let note = row.with_context(|| {
            format!(
                "failed to decode a legacy memory row from {}",
                path.display()
            )
        })?;
        if category_is_eligible(&note.category, include_daily) {
            notes.push(note);
        }
    }
    Ok(notes)
}

fn create_snapshot(config: &Config, sources: &LegacySources) -> Result<SnapshotSet> {
    let migration_id = format!(
        "byori-{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        Uuid::new_v4()
    );
    let migrations_dir = config.workspace_dir.join("migrations");
    fs::create_dir_all(&migrations_dir).with_context(|| {
        format!(
            "failed to create migration directory {}",
            migrations_dir.display()
        )
    })?;
    set_private_dir(&migrations_dir)?;
    ensure_migration_gitignore(&migrations_dir)?;
    let root = migrations_dir.join(migration_id);
    fs::create_dir_all(&root)
        .with_context(|| format!("failed to create migration snapshot {}", root.display()))?;
    set_private_dir(&root)?;

    let mut snapshot = SnapshotSet {
        root: root.clone(),
        files: Vec::new(),
        markdown: Vec::new(),
        memory_snapshot: None,
        memory_db: None,
        graph_dbs: Vec::new(),
    };

    if let Some(source) = &sources.config {
        let destination = root.join("config.toml");
        copy_snapshot_file(source, &destination)?;
        snapshot.files.push(snapshot_file_record(
            "config",
            source,
            Path::new("config.toml"),
        ));
    }

    for source in &sources.markdown {
        let relative = if source.category == "daily" {
            PathBuf::from("markdown/daily").join(
                source
                    .path
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("daily.md")),
            )
        } else {
            PathBuf::from("markdown").join(&source.logical_name)
        };
        let destination = root.join(&relative);
        copy_snapshot_file(&source.path, &destination)?;
        snapshot
            .files
            .push(snapshot_file_record("markdown", &source.path, &relative));
        snapshot.markdown.push(MarkdownSource {
            path: destination,
            logical_name: source.logical_name.clone(),
            category: source.category.clone(),
        });
    }

    if let Some(source) = &sources.memory_snapshot {
        let relative = PathBuf::from("markdown").join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
        let destination = root.join(&relative);
        copy_snapshot_file(source, &destination)?;
        snapshot
            .files
            .push(snapshot_file_record("memory-snapshot", source, &relative));
        snapshot.memory_snapshot = Some(destination);
    }

    if let Some(source) = &sources.memory_db {
        let relative = PathBuf::from("sqlite/brain.db");
        let destination = root.join(&relative);
        sqlite_online_backup(source, &destination)?;
        snapshot
            .files
            .push(snapshot_file_record("sqlite-memory", source, &relative));
        snapshot.memory_db = Some((source.clone(), destination));
    }

    for (index, source) in sources.graph_dbs.iter().enumerate() {
        let relative = PathBuf::from(format!("sqlite/knowledge-{index}.db"));
        let destination = root.join(&relative);
        sqlite_online_backup(&source.path, &destination)?;
        snapshot.files.push(snapshot_file_record(
            "sqlite-knowledge-graph",
            &source.path,
            &relative,
        ));
        snapshot.graph_dbs.push(SnapshotGraphDb {
            logical_name: source.logical_name.clone(),
            original: source.path.clone(),
            snapshot: destination,
        });
    }

    Ok(snapshot)
}

fn ensure_migration_gitignore(migrations_dir: &Path) -> Result<()> {
    const IGNORE_PATTERN: &str = "byori-*/";
    let path = migrations_dir.join(".gitignore");
    if path.is_file() {
        let existing = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if existing.lines().any(|line| line.trim() == IGNORE_PATTERN) {
            return Ok(());
        }
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .with_context(|| format!("failed to open {}", path.display()))?;
        if !existing.is_empty() && !existing.ends_with('\n') {
            writeln!(file)?;
        }
        writeln!(file, "# NaraeClaw knowledge migration snapshots")?;
        writeln!(file, "{IGNORE_PATTERN}")?;
        return Ok(());
    }

    fs::write(
        &path,
        "# NaraeClaw knowledge migration snapshots\nbyori-*/\n",
    )
    .with_context(|| format!("failed to write {}", path.display()))?;
    set_private_file(&path)
}

fn snapshot_file_record(kind: &str, source: &Path, snapshot: &Path) -> SnapshotFile {
    SnapshotFile {
        kind: kind.to_string(),
        source: source.to_string_lossy().into_owned(),
        snapshot: snapshot.to_string_lossy().into_owned(),
    }
}

fn copy_snapshot_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create snapshot directory {}", parent.display()))?;
        set_private_dir(parent)?;
    }
    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to snapshot {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    set_private_file(destination)
}

fn sqlite_online_backup(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create snapshot directory {}", parent.display()))?;
        set_private_dir(parent)?;
    }
    let source_connection = open_read_only(source)?;
    let mut destination_connection = Connection::open(destination)
        .with_context(|| format!("failed to create SQLite snapshot {}", destination.display()))?;
    let backup = Backup::new(&source_connection, &mut destination_connection)
        .with_context(|| format!("failed to initialize backup for {}", source.display()))?;
    backup
        .run_to_completion(128, Duration::from_millis(10), None)
        .with_context(|| format!("failed to back up {}", source.display()))?;
    drop(backup);
    drop(destination_connection);
    set_private_file(destination)
}

#[cfg(unix)]
fn set_private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to secure directory {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_dir(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to secure file {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file(_path: &Path) -> Result<()> {
    Ok(())
}

fn load_snapshot_inventory(snapshot: &SnapshotSet, include_daily: bool) -> Result<LegacyInventory> {
    let mut notes = Vec::new();
    for source in &snapshot.markdown {
        notes.extend(parse_markdown_file(source)?);
    }
    if let Some(memory_snapshot) = &snapshot.memory_snapshot {
        notes.extend(read_legacy_memory_snapshot(memory_snapshot)?);
    }
    if let Some((_original, snapshotted)) = &snapshot.memory_db {
        notes.extend(read_memory_database(
            snapshotted,
            "memory/brain.db",
            include_daily,
        )?);
    }

    let mut graphs = Vec::new();
    for database in &snapshot.graph_dbs {
        let graph = KnowledgeGraph::new(&database.snapshot, usize::MAX).with_context(|| {
            format!(
                "failed to open snapshotted knowledge graph {}",
                database.snapshot.display()
            )
        })?;
        let (nodes, edges) = graph.export_all().with_context(|| {
            format!(
                "failed to export snapshotted knowledge graph {}",
                database.snapshot.display()
            )
        })?;
        graphs.push(LegacyGraphExport {
            source: database.logical_name.clone(),
            original_source: database.original.to_string_lossy().into_owned(),
            snapshot: database.snapshot.to_string_lossy().into_owned(),
            nodes,
            edges,
        });
    }

    let counts = MigrationCounts {
        markdown_notes: notes
            .iter()
            .filter(|note| note.origin == "markdown")
            .count(),
        memory_snapshot_notes: notes
            .iter()
            .filter(|note| note.origin == "memory-snapshot")
            .count(),
        sqlite_memory_notes: notes.iter().filter(|note| note.origin == "sqlite").count(),
        graph_nodes: graphs.iter().map(|graph| graph.nodes.len()).sum(),
        graph_edges: graphs.iter().map(|graph| graph.edges.len()).sum(),
    };
    Ok(LegacyInventory {
        notes,
        graphs,
        counts,
    })
}

fn write_manifest(root: &Path, manifest: &MigrationManifest) -> Result<()> {
    let path = root.join("manifest.json");
    let content = serde_json::to_vec_pretty(manifest).context("failed to serialize manifest")?;
    fs::write(&path, content)
        .with_context(|| format!("failed to write migration manifest {}", path.display()))?;
    set_private_file(&path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ByoriEndpoint {
    node_type: String,
    name: String,
}

async fn apply_inventory(config: &Config, inventory: &LegacyInventory) -> Result<ApplyStats> {
    // Migration is deliberately allowed before cutover. Existing installations
    // can keep legacy memory active (`knowledge.enabled = false`), import and
    // verify ByoriDB, then enable it without a window where durable recall is
    // unavailable. The managed server below still forces the safe profile and
    // workspace-scoped target.
    validate_inventory_for_byori(inventory)?;

    let server = config.byori_mcp_server_config();
    ensure!(
        Path::new(&server.command).is_file(),
        "managed ByoriDB wrapper is missing at {}",
        server.command
    );
    let registry = McpRegistry::connect_all(&[server]).await?;
    ensure!(
        !registry.is_empty(),
        "could not connect to the managed ByoriDB MCP server"
    );
    let tools: HashSet<String> = registry.tool_names().into_iter().collect();
    let missing = missing_tools(&tools, MIGRATION_TOOL_NAMES);
    ensure!(
        missing.is_empty(),
        "ByoriDB safe profile is missing migration tools: {}",
        missing.join(", ")
    );
    ensure!(
        !tools.contains(UNSAFE_TOOL_NAME),
        "ByoriDB unsafe raw query tool is exposed; refuse migration outside the safe profile"
    );

    let mut stats = ApplyStats::default();
    for note in &inventory.notes {
        let body = memory_note_body(note)?;
        if exact_item_matches(
            &registry,
            "note",
            &note.canonical_name,
            &body,
            Some(&note.kind),
        )
        .await?
        {
            stats.notes_unchanged += 1;
            continue;
        }
        call_tool_json(
            &registry,
            "byoridb__memory_remember",
            json!({
                "name": note.canonical_name,
                "kind": note.kind,
                "body": body,
            }),
        )
        .await
        .with_context(|| format!("failed to upsert legacy note {}", note.canonical_name))?;
        stats.notes_upserted += 1;
    }

    let mut graph_endpoints = Vec::with_capacity(inventory.graphs.len());
    for graph in &inventory.graphs {
        let mut endpoints = HashMap::new();
        for node in &graph.nodes {
            let endpoint = graph_endpoint(&graph.source, node);
            let body = graph_node_body(graph, node)?;
            if exact_item_matches(&registry, &endpoint.node_type, &endpoint.name, &body, None)
                .await?
            {
                stats.graph_nodes_unchanged += 1;
            } else {
                call_tool_json(
                    &registry,
                    "byoridb__memory_wiki_upsert",
                    json!({
                        "type": endpoint.node_type,
                        "name": endpoint.name,
                        "body": body,
                    }),
                )
                .await
                .with_context(|| format!("failed to upsert legacy graph node {}", node.id))?;
                stats.graph_nodes_upserted += 1;
            }
            endpoints.insert(node.id.clone(), endpoint);
        }
        graph_endpoints.push(endpoints);
    }

    for (graph, endpoints) in inventory.graphs.iter().zip(&graph_endpoints) {
        for edge in &graph.edges {
            let source = endpoints.get(&edge.from_id).with_context(|| {
                format!("legacy edge refers to missing source node {}", edge.from_id)
            })?;
            let target = endpoints.get(&edge.to_id).with_context(|| {
                format!("legacy edge refers to missing target node {}", edge.to_id)
            })?;
            call_tool_json(
                &registry,
                "byoridb__memory_link",
                json!({
                    "action": "upsert",
                    "relation": "relates_to",
                    "source": {"type": source.node_type, "name": source.name},
                    "target": {"type": target.node_type, "name": target.name},
                }),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to link legacy graph edge {} -[{}]-> {}",
                    edge.from_id,
                    edge.relation.as_str(),
                    edge.to_id
                )
            })?;
            stats.links_upserted += 1;
        }
    }

    Ok(stats)
}

fn validate_inventory_for_byori(inventory: &LegacyInventory) -> Result<()> {
    for note in &inventory.notes {
        let body = memory_note_body(note)?;
        ensure!(
            !note.canonical_name.is_empty()
                && note.canonical_name.chars().count() <= MAX_BYORI_NAME_CHARS,
            "legacy note {} exceeds ByoriDB's {MAX_BYORI_NAME_CHARS}-character name limit; snapshot manifest retains it losslessly",
            note.legacy_id
        );
        ensure!(
            !note.kind.is_empty() && note.kind.chars().count() <= MAX_BYORI_KIND_CHARS,
            "legacy note {} exceeds ByoriDB's {MAX_BYORI_KIND_CHARS}-character kind limit; snapshot manifest retains it losslessly",
            note.legacy_id
        );
        ensure!(
            body.chars().count() <= MAX_BYORI_BODY_CHARS,
            "legacy note {} exceeds ByoriDB's {MAX_BYORI_BODY_CHARS}-character body limit; snapshot manifest retains it losslessly",
            note.legacy_id
        );
    }

    for graph in &inventory.graphs {
        for node in &graph.nodes {
            let endpoint = graph_endpoint(&graph.source, node);
            let body = graph_node_body(graph, node)?;
            ensure!(
                !endpoint.name.is_empty() && endpoint.name.chars().count() <= MAX_BYORI_NAME_CHARS,
                "legacy graph node {} exceeds ByoriDB's {MAX_BYORI_NAME_CHARS}-character name limit; snapshot manifest retains it losslessly",
                node.id
            );
            ensure!(
                body.chars().count() <= MAX_BYORI_BODY_CHARS,
                "legacy graph node {} exceeds ByoriDB's {MAX_BYORI_BODY_CHARS}-character body limit; snapshot manifest retains it losslessly",
                node.id
            );
        }
    }

    Ok(())
}

async fn exact_item_matches(
    registry: &McpRegistry,
    node_type: &str,
    name: &str,
    body: &str,
    kind: Option<&str>,
) -> Result<bool> {
    let response = call_tool_json(
        registry,
        "byoridb__memory_read",
        json!({
            "type": node_type,
            "name": name,
            "limit": 1,
            "include_links": false,
        }),
    )
    .await?;
    Ok(response_item_matches(
        &response, node_type, name, body, kind,
    ))
}

fn response_item_matches(
    response: &Value,
    node_type: &str,
    name: &str,
    body: &str,
    kind: Option<&str>,
) -> bool {
    let Some(item) = response
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return false;
    };
    let same_identity = item.get("type").and_then(Value::as_str) == Some(node_type)
        && item.get("name").and_then(Value::as_str) == Some(name);
    let same_body = item.get("body").and_then(Value::as_str) == Some(body);
    let same_kind =
        kind.is_none_or(|expected| item.get("kind").and_then(Value::as_str) == Some(expected));
    same_identity && same_body && same_kind
}

fn memory_note_body(note: &LegacyMemoryNote) -> Result<String> {
    let metadata =
        serde_json::to_string_pretty(note).context("failed to serialize legacy memory metadata")?;
    Ok(format!(
        "{}\n\n---\nNaraeClaw legacy memory (lossless metadata):\n{metadata}",
        note.content
    ))
}

fn graph_endpoint(source: &str, node: &KnowledgeNode) -> ByoriEndpoint {
    let node_type = byori_graph_type(&node.node_type).to_string();
    let uuid = Uuid::parse_str(&node.id)
        .unwrap_or_else(|_| deterministic_uuid(&[source, node.node_type.as_str(), &node.id]));
    ByoriEndpoint {
        name: format!("{node_type}:narae-{uuid}"),
        node_type,
    }
}

fn byori_graph_type(node_type: &NodeType) -> &'static str {
    match node_type {
        NodeType::Decision => "decision",
        NodeType::Pattern | NodeType::Lesson => "concept",
        NodeType::Expert | NodeType::Technology => "entity",
    }
}

fn deterministic_uuid(parts: &[&str]) -> Uuid {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.len().to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

fn graph_node_body(graph: &LegacyGraphExport, node: &KnowledgeNode) -> Result<String> {
    let incident_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.from_id == node.id || edge.to_id == node.id)
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&json!({
        "format": "naraeclaw-legacy-knowledge-graph-v1",
        "source": graph.source,
        "legacy_node": node,
        "legacy_incident_edges": incident_edges,
    }))
    .context("failed to serialize legacy graph node")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use naraeclaw_memory::knowledge_graph::Relation;

    fn markdown_source() -> MarkdownSource {
        MarkdownSource {
            path: PathBuf::from("MEMORY.md"),
            logical_name: "MEMORY.md".to_string(),
            category: "core".to_string(),
        }
    }

    fn graph_node(id: &str, node_type: NodeType) -> KnowledgeNode {
        KnowledgeNode {
            id: id.to_string(),
            node_type,
            title: "Legacy title".to_string(),
            content: "Legacy content".to_string(),
            tags: vec!["one".to_string(), "two".to_string()],
            created_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 2, 3, 4, 5, 6).unwrap(),
            source_project: Some("naraeclaw".to_string()),
        }
    }

    fn legacy_memory_snapshot_fixture() -> &'static str {
        "# 🧠 NaraeClaw Memory Snapshot\n\n\
         > Auto-generated by NaraeClaw.\n\n\
         **Last exported:** 2026-07-29 10:20:30\n\n\
         **Total core memories:** 2\n\n---\n\n\
         ### 🔑 `identity`\n\n\
         I am NaraeClaw.\n\n\
         *Created: 2026-01-01T00:00:00Z | Updated: 2026-07-01T00:00:00Z*\n\n---\n\n\
         ### 🔑 `rules`\n\n\
         Rule 1: preserve source data.\n\
         Rule 2: keep imports idempotent.\n\n\
         *Created: 2026-02-01T00:00:00Z | Updated: 2026-07-02T00:00:00Z*\n\n---\n"
    }

    #[test]
    fn detects_qdrant_from_legacy_memory_backend() {
        let mut config = Config::default();
        config.memory.backend = " QdRaNt ".to_string();

        let blocker = legacy_qdrant_migration_blocker(&config)
            .expect("the effective qdrant backend must block local-file migration");
        assert!(blocker.contains("[knowledge].enabled = false"));
        assert!(blocker.contains("export the Qdrant collection separately"));
    }

    #[test]
    fn detects_qdrant_from_storage_provider_override() {
        let mut config = Config::default();
        config.memory.backend = "sqlite".to_string();
        config.storage.provider.config.provider = "QDRANT".to_string();

        assert!(legacy_qdrant_migration_blocker(&config).is_some());
    }

    #[test]
    fn local_legacy_backends_do_not_trigger_qdrant_blocker() {
        for backend in ["none", "markdown", "sqlite", "lucid"] {
            let mut config = Config::default();
            config.memory.backend = backend.to_string();
            assert!(
                legacy_qdrant_migration_blocker(&config).is_none(),
                "{backend} should remain eligible for local migration"
            );
        }
    }

    #[tokio::test]
    async fn qdrant_dry_run_and_apply_fail_before_creating_migration_files() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        let mut config = Config::default();
        config.workspace_dir = workspace.clone();
        config.knowledge.enabled = false;
        config.memory.backend = "qdrant".to_string();

        for (dry_run, yes) in [(true, false), (false, true)] {
            let error = handle_migrate(&config, dry_run, false, yes)
                .await
                .expect_err("qdrant migration must fail explicitly");
            let message = error.to_string();
            assert!(message.contains("legacy Qdrant migration is unsupported"));
            assert!(message.contains("[knowledge].enabled = false"));
            assert!(message.contains("export the Qdrant collection separately"));
            assert!(!workspace.join("migrations").exists());
        }
    }

    #[test]
    fn parses_bold_and_plain_markdown_losslessly() {
        let notes = parse_markdown_content(
            &markdown_source(),
            "# Header\n\n- **alpha**: first value\nplain value\n",
        );
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].key, "alpha");
        assert_eq!(notes[0].content, "first value");
        assert_eq!(
            notes[0].raw_markdown.as_deref(),
            Some("- **alpha**: first value")
        );
        assert_eq!(notes[0].line, Some(3));
        assert_eq!(notes[1].content, "plain value");
        assert_eq!(notes[1].line, Some(4));
    }

    #[test]
    fn parses_legacy_memory_snapshot_with_stable_logical_identity() {
        let notes = parse_legacy_memory_snapshot(legacy_memory_snapshot_fixture());
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].key, "identity");
        assert_eq!(notes[0].content, "I am NaraeClaw.");
        assert_eq!(notes[0].source, LEGACY_MEMORY_SNAPSHOT_FILENAME);
        assert_eq!(notes[0].origin, "memory-snapshot");
        assert_eq!(notes[0].created_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert_eq!(notes[0].updated_at.as_deref(), Some("2026-07-01T00:00:00Z"));
        assert!(
            notes[0]
                .raw_markdown
                .as_deref()
                .unwrap()
                .contains("### 🔑 `identity`")
        );
        assert!(notes[1].content.contains("Rule 1"));
        assert!(notes[1].content.contains("Rule 2"));

        let regenerated = legacy_memory_snapshot_fixture().replace(
            "**Last exported:** 2026-07-29 10:20:30",
            "**Last exported:** 2026-08-30 11:22:33",
        );
        let regenerated_notes = parse_legacy_memory_snapshot(&regenerated);
        assert_eq!(notes[0].canonical_name, regenerated_notes[0].canonical_name);
        assert_eq!(
            memory_note_body(&notes[0]).unwrap(),
            memory_note_body(&regenerated_notes[0]).unwrap()
        );
    }

    #[test]
    fn category_filter_excludes_conversation_and_gates_daily() {
        assert!(category_is_eligible("core", false));
        assert!(category_is_eligible("my-custom-category", false));
        assert!(!category_is_eligible("daily", false));
        assert!(category_is_eligible("DAILY", true));
        assert!(!category_is_eligible("Conversation", true));
    }

    #[test]
    fn note_names_are_deterministic_and_length_delimited() {
        let first = deterministic_note_name(&["ab", "c"]);
        assert_eq!(first, deterministic_note_name(&["ab", "c"]));
        assert_ne!(first, deterministic_note_name(&["a", "bc"]));
        assert!(first.starts_with("legacy-memory:"));
    }

    #[test]
    fn parses_removed_legacy_graph_path_from_config() {
        let path = legacy_db_path_from_toml(
            "[knowledge]\nenabled = true\ndb_path = \"~/old-knowledge.db\"\n",
        )
        .unwrap();
        assert_eq!(path.as_deref(), Some("~/old-knowledge.db"));
    }

    #[test]
    fn decodes_mcp_text_envelope() {
        let payload = parse_mcp_tool_payload(
            r#"{"content":[{"type":"text","text":"{\"items\":[],\"space\":\"demo\"}"}]}"#,
        )
        .unwrap();
        assert_eq!(payload["space"], "demo");
        assert_eq!(payload["items"], json!([]));
    }

    #[test]
    fn rejects_mcp_error_envelope() {
        let error = parse_mcp_tool_payload(
            r#"{"content":[{"type":"text","text":"ERROR: unavailable"}],"isError":true}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("ERROR: unavailable"));
    }

    #[test]
    fn idempotency_match_requires_exact_identity_body_and_note_kind() {
        let response = json!({
            "items": [{
                "type": "note",
                "name": "legacy-memory:abc",
                "body": "exact body",
                "kind": "core",
            }]
        });
        assert!(response_item_matches(
            &response,
            "note",
            "legacy-memory:abc",
            "exact body",
            Some("core")
        ));
        assert!(!response_item_matches(
            &response,
            "note",
            "legacy-memory:abc",
            "changed body",
            Some("core")
        ));
        assert!(!response_item_matches(
            &response,
            "note",
            "legacy-memory:abc",
            "exact body",
            Some("daily")
        ));
    }

    #[test]
    fn migration_preflight_rejects_oversized_items_before_apply() {
        let mut notes = parse_markdown_content(&markdown_source(), "small note\n");
        notes[0].content = "x".repeat(MAX_BYORI_BODY_CHARS + 1);
        let inventory = LegacyInventory {
            notes,
            ..LegacyInventory::default()
        };

        let error = validate_inventory_for_byori(&inventory)
            .expect_err("oversized legacy content must fail before MCP writes begin");
        assert!(error.to_string().contains("body limit"));
        assert!(
            error
                .to_string()
                .contains("snapshot manifest retains it losslessly")
        );
    }

    #[test]
    fn graph_mapping_and_body_preserve_legacy_relation() {
        let node = graph_node("legacy-id", NodeType::Pattern);
        let edge = KnowledgeEdge {
            from_id: "legacy-id".to_string(),
            to_id: "target-id".to_string(),
            relation: Relation::Extends,
        };
        let graph = LegacyGraphExport {
            source: "knowledge/configured".to_string(),
            original_source: "/legacy/knowledge.db".to_string(),
            snapshot: "/snapshot/knowledge-0.db".to_string(),
            nodes: vec![node.clone()],
            edges: vec![edge],
        };

        let endpoint = graph_endpoint(&graph.source, &node);
        assert_eq!(endpoint.node_type, "concept");
        assert!(endpoint.name.starts_with("concept:narae-"));
        assert_eq!(endpoint, graph_endpoint(&graph.source, &node));

        let body = graph_node_body(&graph, &node).unwrap();
        assert!(body.contains("legacy-id"));
        assert!(body.contains("target-id"));
        assert!(body.contains("extends"));
        assert!(body.contains("source_project"));
        assert!(body.contains("knowledge/configured"));
        assert!(!body.contains("/legacy/knowledge.db"));
        assert!(!body.contains("/snapshot/knowledge-0.db"));

        let mut relocated = graph.clone();
        relocated.original_source = "/moved/legacy/knowledge.db".to_string();
        relocated.snapshot = "/new-snapshot/knowledge-0.db".to_string();
        assert_eq!(body, graph_node_body(&relocated, &node).unwrap());
    }

    #[test]
    fn existing_uuid_is_normalized_in_canonical_graph_name() {
        let uuid = "67f4f116-2c67-4a52-b270-d89d519a28a2";
        let node = graph_node(uuid, NodeType::Decision);
        let endpoint = graph_endpoint("/legacy/knowledge.db", &node);
        assert_eq!(endpoint.node_type, "decision");
        assert_eq!(endpoint.name, format!("decision:narae-{uuid}"));
    }

    #[test]
    fn sqlite_online_backup_contains_committed_wal_data() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source.db");
        let destination = temporary.path().join("snapshot.db");
        let connection = Connection::open(&source).unwrap();
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        connection
            .execute_batch(
                "CREATE TABLE sample (value TEXT NOT NULL);\n\
                 INSERT INTO sample (value) VALUES ('captured');",
            )
            .unwrap();

        sqlite_online_backup(&source, &destination).unwrap();

        let snapshotted = open_read_only(&destination).unwrap();
        let value: String = snapshotted
            .query_row("SELECT value FROM sample", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "captured");
        let source_value: String = connection
            .query_row("SELECT value FROM sample", [], |row| row.get(0))
            .unwrap();
        assert_eq!(source_value, "captured");
    }

    #[test]
    fn sqlite_note_identity_uses_logical_source_not_physical_path() {
        let temporary = tempfile::tempdir().unwrap();
        let first_path = temporary.path().join("first/brain.db");
        let second_path = temporary.path().join("moved/brain.db");
        for path in [&first_path, &second_path] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let connection = Connection::open(path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE memories (\n\
                         id TEXT PRIMARY KEY, key TEXT NOT NULL, content TEXT NOT NULL,\n\
                         category TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\n\
                     );\n\
                     INSERT INTO memories VALUES\n\
                         ('same-id', 'same-key', 'same-body', 'core', 'created', 'updated');",
                )
                .unwrap();
        }

        let first = read_memory_database(&first_path, "memory/brain.db", false).unwrap();
        let second = read_memory_database(&second_path, "memory/brain.db", false).unwrap();
        assert_eq!(first[0].canonical_name, second[0].canonical_name);
        assert_eq!(first[0].source, "memory/brain.db");
        assert_eq!(second[0].source, "memory/brain.db");
    }

    #[test]
    fn migration_gitignore_preserves_existing_content_and_excludes_snapshots() {
        let temporary = tempfile::tempdir().unwrap();
        let migrations = temporary.path().join("migrations");
        fs::create_dir_all(&migrations).unwrap();
        let path = migrations.join(".gitignore");
        fs::write(&path, "keep-this-line\n").unwrap();

        ensure_migration_gitignore(&migrations).unwrap();
        ensure_migration_gitignore(&migrations).unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("keep-this-line"));
        assert_eq!(
            content
                .lines()
                .filter(|line| line.trim() == "byori-*/")
                .count(),
            1
        );
    }

    #[test]
    fn snapshot_uses_private_expected_layout_without_rewriting_sources() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        fs::create_dir_all(workspace.join("memory")).unwrap();
        let config_path = temporary.path().join("config.toml");
        let core_path = workspace.join("MEMORY.md");
        let daily_path = workspace.join("memory/2026-07-29.md");
        let memory_snapshot_path = workspace.join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
        let memory_db = workspace.join("memory/brain.db");
        let graph_db = temporary.path().join("legacy-knowledge.db");
        fs::write(&config_path, "[memory]\nbackend = \"sqlite\"\n").unwrap();
        fs::write(&core_path, "# Memory\n\n- core\n").unwrap();
        fs::write(&daily_path, "# Daily\n\n- daily\n").unwrap();
        fs::write(&memory_snapshot_path, legacy_memory_snapshot_fixture()).unwrap();
        for path in [&memory_db, &graph_db] {
            let connection = Connection::open(path).unwrap();
            connection
                .execute_batch("CREATE TABLE marker (value TEXT NOT NULL);")
                .unwrap();
        }
        let config_before = fs::read(&config_path).unwrap();
        let core_before = fs::read(&core_path).unwrap();
        let daily_before = fs::read(&daily_path).unwrap();
        let memory_snapshot_before = fs::read(&memory_snapshot_path).unwrap();

        let mut config = Config::default();
        config.workspace_dir = workspace.clone();
        config.config_path = config_path.clone();
        let sources = LegacySources {
            config: Some(config_path.clone()),
            markdown: vec![
                MarkdownSource {
                    path: core_path.clone(),
                    logical_name: "MEMORY.md".to_string(),
                    category: "core".to_string(),
                },
                MarkdownSource {
                    path: daily_path.clone(),
                    logical_name: "memory/2026-07-29.md".to_string(),
                    category: "daily".to_string(),
                },
            ],
            memory_snapshot: Some(memory_snapshot_path.clone()),
            memory_db: Some(memory_db),
            graph_dbs: vec![LegacyGraphSource {
                path: graph_db,
                logical_name: "knowledge/configured".to_string(),
            }],
        };

        let snapshot = create_snapshot(&config, &sources).unwrap();

        assert!(
            snapshot
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("byori-")
        );
        for relative in [
            "config.toml",
            "markdown/MEMORY.md",
            "markdown/MEMORY_SNAPSHOT.md",
            "markdown/daily/2026-07-29.md",
            "sqlite/brain.db",
            "sqlite/knowledge-0.db",
        ] {
            assert!(snapshot.root.join(relative).is_file(), "missing {relative}");
        }
        assert!(workspace.join("migrations/.gitignore").is_file());
        assert_eq!(snapshot.graph_dbs[0].logical_name, "knowledge/configured");
        assert_eq!(fs::read(config_path).unwrap(), config_before);
        assert_eq!(fs::read(core_path).unwrap(), core_before);
        assert_eq!(fs::read(daily_path).unwrap(), daily_before);
        assert_eq!(
            fs::read(memory_snapshot_path).unwrap(),
            memory_snapshot_before
        );
    }

    #[test]
    fn memory_snapshot_is_counted_snapshotted_and_loaded_for_import() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let source_path = workspace.join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
        fs::write(&source_path, legacy_memory_snapshot_fixture()).unwrap();
        let source_before = fs::read(&source_path).unwrap();
        let mut config = Config::default();
        config.workspace_dir = workspace.clone();
        config.config_path = temporary.path().join("missing-config.toml");

        let sources = discover_legacy_sources(&config, false).unwrap();
        assert_eq!(
            sources.memory_snapshot.as_deref(),
            Some(source_path.as_path())
        );
        let counts = count_legacy_sources(&sources, false).unwrap();
        assert_eq!(counts.memory_snapshot_notes, 2);
        assert_eq!(counts.total_items(), 2);

        let snapshot = create_snapshot(&config, &sources).unwrap();
        let snapshotted_path = snapshot
            .root
            .join("markdown")
            .join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
        assert_eq!(fs::read(&snapshotted_path).unwrap(), source_before);
        let inventory = load_snapshot_inventory(&snapshot, false).unwrap();
        assert_eq!(inventory.counts.memory_snapshot_notes, 2);
        assert_eq!(inventory.notes.len(), 2);
        assert!(
            inventory
                .notes
                .iter()
                .all(|note| note.origin == "memory-snapshot")
        );
        assert_eq!(fs::read(source_path).unwrap(), source_before);
    }

    #[test]
    fn brain_database_suppresses_memory_snapshot_fallback_source() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        fs::create_dir_all(workspace.join("memory")).unwrap();
        let snapshot_path = workspace.join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
        fs::write(&snapshot_path, legacy_memory_snapshot_fixture()).unwrap();
        let brain_path = workspace.join("memory/brain.db");
        let connection = Connection::open(&brain_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE memories (\n\
                     id TEXT PRIMARY KEY, key TEXT NOT NULL, content TEXT NOT NULL,\n\
                     category TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL\n\
                 );\n\
                 INSERT INTO memories VALUES\n\
                     ('db-id', 'identity', 'authoritative DB value', 'core', 'created', 'updated');",
            )
            .unwrap();
        drop(connection);
        let mut config = Config::default();
        config.workspace_dir = workspace;
        config.config_path = temporary.path().join("missing-config.toml");

        let sources = discover_legacy_sources(&config, false).unwrap();
        assert_eq!(sources.memory_db.as_deref(), Some(brain_path.as_path()));
        assert!(sources.memory_snapshot.is_none());
        let counts = count_legacy_sources(&sources, false).unwrap();
        assert_eq!(counts.sqlite_memory_notes, 1);
        assert_eq!(counts.memory_snapshot_notes, 0);
        assert_eq!(counts.total_items(), 1);
    }

    #[tokio::test]
    async fn dry_run_does_not_create_snapshot_or_modify_sources() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        fs::create_dir_all(workspace.join("memory")).unwrap();
        let memory_path = workspace.join("MEMORY.md");
        let daily_path = workspace.join("memory/2026-07-29.md");
        let memory_snapshot_path = workspace.join(LEGACY_MEMORY_SNAPSHOT_FILENAME);
        fs::write(&memory_path, "# Memory\n\n- **stable**: unchanged\n").unwrap();
        fs::write(&daily_path, "# Daily\n\n- daily entry\n").unwrap();
        fs::write(&memory_snapshot_path, legacy_memory_snapshot_fixture()).unwrap();
        let config_path = temporary.path().join("config.toml");
        fs::write(&config_path, "# migration dry-run fixture\n").unwrap();

        let mut config = Config::default();
        config.workspace_dir = workspace.clone();
        config.config_path = config_path.clone();
        let memory_before = fs::read(&memory_path).unwrap();
        let daily_before = fs::read(&daily_path).unwrap();
        let memory_snapshot_before = fs::read(&memory_snapshot_path).unwrap();
        let config_before = fs::read(&config_path).unwrap();

        handle_migrate(&config, true, true, false).await.unwrap();

        assert!(!workspace.join("migrations").exists());
        assert_eq!(fs::read(&memory_path).unwrap(), memory_before);
        assert_eq!(fs::read(&daily_path).unwrap(), daily_before);
        assert_eq!(
            fs::read(&memory_snapshot_path).unwrap(),
            memory_snapshot_before
        );
        assert_eq!(fs::read(&config_path).unwrap(), config_before);
    }

    #[tokio::test]
    async fn apply_without_yes_fails_before_creating_snapshot() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("MEMORY.md"), "- requires confirmation\n").unwrap();
        let mut config = Config::default();
        config.workspace_dir = workspace.clone();
        config.config_path = temporary.path().join("missing-config.toml");

        let error = handle_migrate(&config, false, false, false)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("requires --yes"));
        assert!(!workspace.join("migrations").exists());
    }
}
