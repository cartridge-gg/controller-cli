use crate::{
    config::Config,
    error::{CliError, Result},
    output::OutputFormatter,
};
use account_sdk::storage::{filestorage::FileSystemBackend, StorageBackend, StorageValue};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize)]
pub struct ListOutput {
    pub total_count: u64,
    pub page: u32,
    pub total_pages: u32,
    pub sessions: Vec<SessionEntry>,
}

#[derive(Serialize)]
pub struct SessionEntry {
    pub guid: String,
    pub app: String,
    pub expires_at: u64,
    pub expires_in: String,
    pub is_current: bool,
}

pub async fn execute(
    config: &Config,
    formatter: &dyn OutputFormatter,
    chain_id: Option<String>,
    limit: u32,
    page: u32,
) -> Result<()> {
    let page = page.max(1);

    let storage_path = PathBuf::from(shellexpand::tilde(&config.session.storage_path).to_string());
    let backend = FileSystemBackend::new(storage_path);
    let controller = backend
        .controller()
        .ok()
        .flatten()
        .ok_or(CliError::NoSession)?;

    let address = format!("0x{:x}", controller.address);
    let chain_id = chain_id.unwrap_or_else(|| {
        starknet::core::utils::parse_cairo_short_string(&controller.chain_id)
            .unwrap_or_else(|_| format!("0x{:x}", controller.chain_id))
    });

    let current_guid = backend
        .get("session_key_guid")
        .ok()
        .flatten()
        .and_then(|v| match v {
            StorageValue::String(s) => Some(s),
            _ => None,
        });

    // Walk through pages to reach the requested one
    let mut result =
        query_sessions(&config.session.api_url, &address, &chain_id, limit, None).await?;

    for _ in 1..page {
        match result.page_info.end_cursor {
            Some(ref c) => {
                result =
                    query_sessions(&config.session.api_url, &address, &chain_id, limit, Some(c))
                        .await?;
            }
            None => break,
        }
    }
    let sessions: Vec<SessionEntry> = result
        .edges
        .iter()
        .map(|edge| {
            let app = edge
                .node
                .app_id
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string();
            SessionEntry {
                guid: edge.node.session_key_guid.clone(),
                app,
                expires_at: edge.node.expires_at,
                expires_in: format_expires(edge.node.expires_at),
                is_current: current_guid.as_deref() == Some(&edge.node.session_key_guid),
            }
        })
        .collect();

    let total_pages = (result.total_count as u32 + limit - 1) / limit;
    let has_next = page < total_pages;

    let output = ListOutput {
        total_count: result.total_count,
        page,
        total_pages,
        sessions,
    };

    if config.cli.json_output {
        formatter.success(&output);
    } else {
        formatter.info(&format!(
            "Active sessions: {} ({})",
            result.total_count, chain_id
        ));

        if output.sessions.is_empty() {
            formatter.info("No sessions found.");
        } else {
            println!();
            println!("  {:<68} {:<24} {}", "SESSION ID", "APP", "EXPIRES");
            println!("  {}", "-".repeat(104));

            for s in &output.sessions {
                let app_display = if s.app.len() > 22 {
                    format!("{}...", &s.app[..19])
                } else {
                    s.app.clone()
                };
                let marker = if s.is_current { " <-- Current" } else { "" };
                println!(
                    "  {:<68} {:<24} {}{}",
                    s.guid, app_display, s.expires_in, marker
                );
            }
            println!();
        }

        if total_pages > 1 {
            formatter.info(&format!("Page {page}/{total_pages}"));
        }
        if has_next {
            formatter.info(&format!("Use --page {} to see more.", page + 1));
        }
    }

    Ok(())
}

fn format_expires(ts: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if ts <= now {
        return "expired".to_string();
    }

    let remaining = ts - now;
    let days = remaining / 86400;
    let hours = (remaining % 86400) / 3600;
    let minutes = (remaining % 3600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

// GraphQL types

#[derive(Deserialize)]
struct SessionsConnection {
    #[serde(rename = "totalCount")]
    total_count: u64,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
    edges: Vec<SessionEdge>,
}

#[derive(Deserialize)]
struct PageInfo {
    #[serde(rename = "endCursor")]
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct SessionEdge {
    node: SessionNode,
}

#[derive(Deserialize)]
struct SessionNode {
    #[serde(rename = "appID")]
    app_id: String,
    #[serde(rename = "sessionKeyGUID")]
    session_key_guid: String,
    #[serde(rename = "expiresAt")]
    expires_at: u64,
}

async fn query_sessions(
    api_url: &str,
    address: &str,
    chain_id: &str,
    first: u32,
    after: Option<&str>,
) -> Result<SessionsConnection> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CliError::ApiError(format!("Failed to build HTTP client: {e}")))?;

    let query = r#"
        query ListSessions($address: String!, $chainID: String!, $first: Int!, $after: Cursor) {
            sessions(
                where: {
                    hasControllerWith: { address: $address }
                    isRevoked: false
                    chainID: $chainID
                }
                orderBy: { field: CREATED_AT, direction: DESC }
                first: $first
                after: $after
            ) {
                totalCount
                pageInfo {
                    endCursor
                }
                edges {
                    node {
                        appID
                        sessionKeyGUID
                        expiresAt
                    }
                }
            }
        }
    "#;

    #[derive(Serialize)]
    struct Variables<'a> {
        address: &'a str,
        #[serde(rename = "chainID")]
        chain_id: &'a str,
        first: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        after: Option<&'a str>,
    }

    #[derive(Serialize)]
    struct GraphQLRequest<'a> {
        query: &'a str,
        variables: Variables<'a>,
    }

    #[derive(Deserialize)]
    struct GraphQLResponse {
        data: Option<GraphQLData>,
        errors: Option<Vec<GraphQLError>>,
    }

    #[derive(Deserialize)]
    struct GraphQLData {
        sessions: SessionsConnection,
    }

    #[derive(Deserialize)]
    struct GraphQLError {
        message: String,
    }

    let request = GraphQLRequest {
        query,
        variables: Variables {
            address,
            chain_id,
            first,
            after,
        },
    };

    let response = client
        .post(api_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to query sessions: {e}")))?;

    if !response.status().is_success() {
        return Err(CliError::ApiError(format!(
            "API returned error status: {}",
            response.status()
        )));
    }

    let graphql_response: GraphQLResponse = response
        .json()
        .await
        .map_err(|e| CliError::ApiError(format!("Failed to parse API response: {e}")))?;

    if let Some(errors) = graphql_response.errors {
        let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        return Err(CliError::ApiError(format!(
            "GraphQL errors: {}",
            messages.join(", ")
        )));
    }

    graphql_response
        .data
        .map(|d| d.sessions)
        .ok_or_else(|| CliError::ApiError("No data in API response".to_string()))
}
