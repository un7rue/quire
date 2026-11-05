//! Database connections: the tables a connection offers, and SQL run against
//! it (read-only) with a preview of the result. A data source on a
//! connection stores only the connection's id and its SQL; it loads through
//! the connection's own loader (DuckDB, SQLite, PostgreSQL or ClickHouse).

use std::path::Path;
use std::time::Instant;

use serde_json::{json, Value as Json};

use crate::data::{self, LoadCtx, Table};
use crate::model::{Connection, Doc, Source};
use crate::python::PyEngine;

/// The source a data source on a connection loads as.
pub fn resolve(doc: &Doc, src: &Source) -> Result<Source, String> {
    match src {
        Source::Db { connection, query } => {
            let c = doc.connections.iter().find(|c| c.id == *connection).ok_or("This data source’s connection was removed.")?;
            as_source(c, query)
        }
        other => Ok(other.clone()),
    }
}

/// A query on a connection as one of the loaders' sources.
pub fn as_source(c: &Connection, query: &str) -> Result<Source, String> {
    Ok(match c.kind.as_str() {
        "duckdb" | "sqlite" if c.path.trim().is_empty() => return Err("Choose the database file of this connection.".into()),
        "duckdb" => Source::Duckdb { path: c.path.clone(), query: query.into() },
        "sqlite" => Source::Sqlite { path: c.path.clone(), query: query.into() },
        "postgres" if c.url.trim().is_empty() => return Err("Enter the server address of this connection.".into()),
        "postgres" => Source::Postgres { url: c.url.clone(), query: query.into() },
        "clickhouse" if c.url.trim().is_empty() => return Err("Enter the server address of this connection.".into()),
        "clickhouse" => Source::Clickhouse { url: c.url.clone(), query: query.into() },
        k => return Err(format!("Unknown connection type “{k}”.")),
    })
}

fn run_raw(c: &Connection, sql: &str, base: Option<&Path>, py: &mut PyEngine) -> Result<Table, String> {
    let values = Default::default();
    let mut ctx = LoadCtx { base_dir: base, range: None, values: &values, py, sources: None };
    // the workspace shows timestamps with their time
    data::with_times(|| data::load(&as_source(c, sql)?, &mut ctx))
}

/// Statements that change data: refused, whatever the connection allows
/// (DuckDB and SQLite files are opened read-only anyway).
fn writes(sql: &str) -> bool {
    let low = sql.to_lowercase();
    ["insert", "update", "delete", "drop", "alter", "create", "truncate", "grant", "revoke", "copy", "attach", "merge", "call", "vacuum"]
        .iter()
        .any(|w| low.split(|c: char| !c.is_alphanumeric() && c != '_').any(|t| t == *w))
}

/// The tables and views of a connection: schema, name, kind, row count and
/// columns with their types.
pub fn tables(c: &Connection, base: Option<&Path>, py: &mut PyEngine) -> Result<Json, String> {
    let (list_sql, count_of): (&str, fn(&str, &str) -> String) = match c.kind.as_str() {
        "duckdb" => (
            "select c.table_schema, c.table_name, coalesce(t.table_type, 'BASE TABLE') as kind,
                    string_agg(c.column_name || chr(9) || c.data_type, chr(10) order by c.ordinal_position) as cols
             from information_schema.columns c
             left join information_schema.tables t on t.table_schema = c.table_schema and t.table_name = c.table_name
             where c.table_schema not in ('information_schema', 'pg_catalog')
             group by all order by 1, 2",
            |s, t| format!("select count(*) from {}.{}", data::quote_ident(s), data::quote_ident(t)),
        ),
        "sqlite" => (
            "select 'main' as table_schema, m.name as table_name, upper(m.type) as kind,
                    group_concat(p.name || char(9) || p.type, char(10)) as cols
             from sqlite_master m join pragma_table_info(m.name) p
             where m.type in ('table', 'view') and m.name not like 'sqlite_%'
             group by m.name order by m.name",
            |_, t| format!("select count(*) from {}", data::quote_ident(t)),
        ),
        "postgres" => (
            "select c.table_schema, c.table_name, t.table_type as kind,
                    string_agg(c.column_name || chr(9) || c.data_type, chr(10) order by c.ordinal_position) as cols
             from information_schema.columns c
             join information_schema.tables t on t.table_schema = c.table_schema and t.table_name = c.table_name
             where c.table_schema not in ('information_schema', 'pg_catalog')
             group by 1, 2, 3 order by 1, 2",
            |s, t| format!("select count(*) from {}.{}", data::quote_ident(s), data::quote_ident(t)),
        ),
        // the row counts ClickHouse keeps for its tables come with the list (no count queries)
        "clickhouse" => (
            "select c.database as table_schema, c.table as table_name,
                    if(t.engine like '%View%', 'VIEW', 'BASE TABLE') as kind,
                    arrayStringConcat(arrayMap(x -> x.2, arraySort(x -> x.1, groupArray((c.position, concat(c.name, char(9), c.type))))), char(10)) as cols,
                    any(t.total_rows) as n
             from system.columns c
             join system.tables t on t.database = c.database and t.name = c.table
             where c.database not in ('system', 'INFORMATION_SCHEMA', 'information_schema')
             group by table_schema, table_name, kind order by table_schema, table_name",
            |s, t| format!("select count() from {}.{}", data::quote_ident(s), data::quote_ident(t)),
        ),
        k => return Err(format!("Unknown connection type “{k}”.")),
    };
    let t = run_raw(c, list_sql, base, py)?;
    let mut out = Vec::new();
    let mut counts = Vec::new();
    for r in &t.rows {
        let (schema, name, kind, cols) = (r[0].text(), r[1].text(), r[2].text(), r[3].text());
        let columns: Vec<Json> = cols
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(n, ty)| json!({"name": n, "type": ty.to_lowercase()}))
            .collect();
        counts.push(count_of(&schema, &name));
        // ClickHouse lists its row counts (none for views)
        let rows = r.get(4).and_then(|v| v.num()).map(|n| json!(n as i64)).unwrap_or(Json::Null);
        out.push(json!({"schema": schema, "name": name, "kind": if kind.contains("VIEW") { "view" } else { "table" }, "columns": columns, "rows": rows}));
    }
    // row counts in one round trip (PostgreSQL tables can be large: counted only when there are few)
    if !counts.is_empty() && c.kind != "clickhouse" && (c.kind != "postgres" || counts.len() <= 40) {
        let sql = counts.iter().enumerate().map(|(i, q)| format!("select {i} as i, ({q}) as n")).collect::<Vec<_>>().join(" union all ");
        if let Ok(n) = run_raw(c, &sql, base, py) {
            for r in &n.rows {
                if let (Some(i), Some(v)) = (r[0].num(), r[1].num()) {
                    if let Some(o) = out.get_mut(i as usize) {
                        o["rows"] = json!(v as i64);
                    }
                }
            }
        }
    }
    Ok(json!(out))
}

/// Runs a query on a connection: the first `limit` rows, how many there are
/// in all and how long it took. `:data_start`, `:data_end`, the period and the report's
/// parameters (`:client`…) are filled in as they will be in the report.
pub fn run(c: &Connection, sql: &str, limit: usize, doc: Option<&Doc>, base: Option<&Path>, py: &mut PyEngine) -> Result<Json, String> {
    let sql = sql.trim().trim_end_matches(';').trim();
    if sql.is_empty() {
        return Err("Write a query first, e.g. select * from clients.".into());
    }
    if writes(sql) {
        return Err("Only queries that read data can run here (SELECT, WITH …). Connections are read-only.".into());
    }
    let empty = Default::default();
    let (range, values) = match doc {
        // the range the report reads; without dated sources, the report period
        Some(d) => (crate::query::Engine::needed_range(d).or_else(|| Some(crate::period::period(&d.params))), &d.params.values),
        None => (None, &empty),
    };
    let values = match doc {
        Some(d) => crate::query::sql_values(d),
        None => values.clone(),
    };
    let (bound, named) = data::bind_sql_named(sql, range, &values);
    let started = Instant::now();
    let mut t = run_raw(c, &format!("select * from (\n{bound}\n) as q limit {}", limit + 1), base, py)?;
    // `select :client` shows as :client, not as the client's name
    data::name_param_columns(&mut t, &named);
    let ms = started.elapsed().as_millis() as u64;
    let total = run_raw(c, &format!("select count(*) from (\n{bound}\n) as q"), base, py)
        .ok()
        .and_then(|n| n.rows.first().and_then(|r| r.first()).and_then(|v| v.num()))
        .map(|n| n as u64);
    let fields = t.fields();
    let shown: Vec<Json> = t.rows.iter().take(limit).map(|r| Json::Array(r.iter().map(|v| v.to_json()).collect())).collect();
    Ok(json!({
        "columns": fields.iter().map(|f| json!({"name": f.name, "kind": f.kind})).collect::<Vec<_>>(),
        "rows": shown,
        "total": total,
        "more": t.rows.len() > limit,
        "ms": ms,
        "sql": bound,
    }))
}

/// The demo database next to the app's sources, when it is there.
pub fn demo_path() -> Option<String> {
    // QUIRE_DEMO_DB: where an installed server keeps it (the Docker image); else the source tree's
    let p = std::env::var("QUIRE_DEMO_DB").map(std::path::PathBuf::from).unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("demo.duckdb"));
    p.exists().then(|| p.display().to_string())
}

/// One edited cell: the row (its rowid, or ctid on PostgreSQL), the column,
/// the new value and the column's type (the value is cast to it).
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct CellEdit {
    pub key: Json,
    pub column: String,
    pub value: Json,
    #[serde(default)]
    pub r#type: String,
}

/// A value as an SQL literal, cast to the column's type.
fn literal(v: &Json, ty: &str) -> Result<String, String> {
    let lit = match v {
        Json::Null => return Ok("NULL".into()),
        Json::Bool(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
        Json::Number(n) => n.to_string(),
        Json::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => return Err("Only plain values can be saved.".into()),
    };
    // a type name only (letters, digits, spaces, brackets, commas)
    let ty = ty.trim();
    if ty.is_empty() || !ty.chars().all(|c| c.is_ascii_alphanumeric() || " _(),[]".contains(c)) {
        return Ok(lit);
    }
    Ok(format!("CAST({lit} AS {ty})"))
}

/// Writes edited cells to one table, all or nothing. Each edit must change
/// exactly one row. Returns how many cells were saved.
pub fn apply(c: &Connection, schema: &str, table: &str, edits: &[CellEdit], base: Option<&Path>) -> Result<usize, String> {
    if edits.is_empty() {
        return Ok(0);
    }
    let t = if schema.is_empty() || schema == "main" || schema == "public" { data::quote_ident(table) } else { format!("{}.{}", data::quote_ident(schema), data::quote_ident(table)) };
    let pg = c.kind == "postgres";
    let mut stmts = Vec::new();
    for e in edits {
        let key = match (&e.key, pg) {
            (Json::Number(n), false) => format!("rowid = {n}"),
            (Json::String(s), true) => format!("ctid = '{}'::tid", s.replace('\'', "''")),
            _ => return Err("A row without its row id can't be saved; run the query again.".into()),
        };
        stmts.push(format!("UPDATE {t} SET {} = {} WHERE {key}", data::quote_ident(&e.column), literal(&e.value, &e.r#type)?));
    }
    let one = |n: usize, i: usize| if n == 1 { Ok(()) } else { Err(format!("Change {} would update {n} rows instead of one; nothing was saved.", i + 1)) };
    match c.kind.as_str() {
        "duckdb" => {
            let path = crate::data::resolve_path(&c.path, base);
            crate::data::check_file(&path)?;
            // the shared read-only instance lets go first; the next read opens the file afresh
            crate::data::release_db(&path);
            let mut conn = duckdb::Connection::open(&path).map_err(|e| format!("Could not open {} for writing: {e}", path.display()))?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for (i, s) in stmts.iter().enumerate() {
                one(tx.execute(s, []).map_err(|e| format!("SQL error: {e}"))?, i)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        "sqlite" => {
            let path = crate::data::resolve_path(&c.path, base);
            crate::data::check_file(&path)?;
            let mut conn = rusqlite::Connection::open(&path).map_err(|e| format!("Could not open {} for writing: {e}", path.display()))?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            for (i, s) in stmts.iter().enumerate() {
                one(tx.execute(s, []).map_err(|e| format!("SQL error: {e}"))?, i)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        "postgres" => {
            let url = c.url.clone();
            crate::data::check_host(&url)?;
            std::thread::spawn(move || -> Result<(), String> {
                let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| format!("Could not connect: {e}"))?;
                let mut tx = client.transaction().map_err(|e| e.to_string())?;
                for (i, s) in stmts.iter().enumerate() {
                    one(tx.execute(s.as_str(), &[]).map_err(|e| format!("SQL error: {e}"))? as usize, i)?;
                }
                tx.commit().map_err(|e| e.to_string())
            })
            .join()
            .map_err(|_| "The database connection failed.".to_string())??;
        }
        "clickhouse" => return Err("ClickHouse connections are read-only here: values can't be changed from the workspace.".into()),
        k => return Err(format!("Unknown connection type “{k}”.")),
    }
    Ok(edits.len())
}

/// The `:names` a query can use, with the values they take now: the range
/// the report reads, the period, and the report's parameters.
pub fn params(doc: &Doc) -> Json {
    let mut out = Vec::new();
    let (s, e) = match crate::query::Engine::needed_range(doc).or_else(|| Some(crate::period::period(&doc.params))) {
        Some(r) => (r.start.format("%Y-%m-%d").to_string(), r.end.format("%Y-%m-%d").to_string()),
        None => ("0001-01-01".into(), "9999-12-31".into()),
    };
    out.push(json!({"name": "data_start", "value": s, "about": "First day of the data to load: the period, plus what comparisons and history need"}));
    out.push(json!({"name": "data_end", "value": e, "about": "Last day of the data to load"}));
    if let Some(r) = crate::period::window(&doc.params, "period") {
        out.push(json!({"name": "period_start", "value": r.start.format("%Y-%m-%d").to_string(), "about": "First day of the report period"}));
        out.push(json!({"name": "period_end", "value": r.end.format("%Y-%m-%d").to_string(), "about": "Last day of the report period"}));
    }
    for (n, v, kind) in crate::vm::sql_vars(doc) {
        if doc.param_defs.iter().any(|p| p.name == n) {
            continue;
        }
        let about = doc.variables.iter().find(|x| x.name.trim() == n).map(|x| if x.description.is_empty() { x.label.clone() } else { x.description.clone() }).unwrap_or_default();
        let what = match kind.as_str() { "sql" => "Variable · SQL", "data" => "Variable · from data", _ => "Variable" };
        out.push(json!({"name": n, "value": v, "about": if about.is_empty() { what.to_string() } else { format!("{what} · {about}") }, "var": true}));
    }
    // variables whose names SQL can't use: shown, so the designer knows why
    let usable: Vec<String> = crate::vm::sql_vars(doc).into_iter().map(|(n, _, _)| n).collect();
    for v in doc.variables.iter().filter(|v| !v.name.trim().is_empty() && !usable.iter().any(|n| n == v.name.trim())) {
        if doc.param_defs.iter().any(|p| p.name == v.name.trim()) {
            continue;
        }
        out.push(json!({"name": v.name.trim(), "value": "", "about": "Rename it to a plain word (latin letters, digits, _) to use it in SQL", "var": true, "bad": true}));
    }
    for p in &doc.param_defs {
        let v = doc.params.values.get(&p.name).cloned().unwrap_or_else(|| p.default.clone());
        let v = v.replace('\u{1f}', ", ");
        let label = if p.label.is_empty() { "Report parameter".to_string() } else { format!("Report parameter · {}", p.label) };
        out.push(json!({"name": p.name, "value": v, "about": label, "param": true}));
    }
    json!(out)
}
