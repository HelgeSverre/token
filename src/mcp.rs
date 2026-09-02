//! Stdio MCP server backed by Token's local automation endpoint.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use serde::Deserialize;

use crate::automation::{self, AutomationRequest, Target};

// Every tool takes an optional `instance` (the editor's process id, as
// listed by `list_instances`); omitted means the most recently focused
// editor.

#[derive(Debug, Deserialize, JsonSchema)]
struct InstanceParams {
    instance: Option<u32>,
}

fn target(instance: Option<u32>) -> Target {
    instance.map_or(Target::Default, Target::Instance)
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TextParams {
    text: String,
    instance: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScrollParams {
    lines: i32,
    instance: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ProfileParams {
    frames: usize,
    instance: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PositionParams {
    line: usize,
    column: usize,
    instance: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SelectionParams {
    anchor_line: usize,
    anchor_column: usize,
    head_line: usize,
    head_column: usize,
    instance: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ActionParams {
    name: String,
    instance: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct OpenPathsParams {
    /// Paths to open; `file:line[:column]` suffixes position the cursor
    /// (1-indexed). Directories go to the editor already showing that
    /// workspace or get their own window.
    paths: Vec<String>,
    instance: Option<u32>,
}

#[derive(Debug, Clone)]
struct TokenMcp;

#[tool_router]
impl TokenMcp {
    #[tool(
        description = "Inspect the running Token editor and its latest performance measurements"
    )]
    async fn get_state(
        &self,
        Parameters(InstanceParams { instance }): Parameters<InstanceParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::State).await
    }

    #[tool(
        description = "List the running Token editors: instance_id (process id), workspace_root, document_name, and focused_at_ms; the most recently focused comes first and is the default target"
    )]
    async fn list_instances(&self) -> CallToolResult {
        let infos: Vec<automation::InstanceInfo> =
            tokio::task::spawn_blocking(automation::discover)
                .await
                .map(|instances| instances.into_iter().map(|i| i.info).collect())
                .unwrap_or_default();
        match serde_json::to_value(&infos) {
            Ok(value) => CallToolResult::structured(serde_json::json!({ "instances": value })),
            Err(error) => tool_error(error.to_string()),
        }
    }

    #[tool(description = "Read the active Token document when it is at most 3 MiB")]
    async fn get_document(
        &self,
        Parameters(InstanceParams { instance }): Parameters<InstanceParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::Document).await
    }

    #[tool(description = "List named actions currently bound in the running Token editor")]
    async fn list_actions(
        &self,
        Parameters(InstanceParams { instance }): Parameters<InstanceParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::Actions).await
    }

    #[tool(description = "Open files in the running Token editor, optionally at file:line:column")]
    async fn open_paths(
        &self,
        Parameters(OpenPathsParams { paths, instance }): Parameters<OpenPathsParams>,
    ) -> CallToolResult {
        let paths: Vec<automation::OpenPath> = paths
            .iter()
            .map(|path| automation::OpenPath::from_arg(std::path::Path::new(path)))
            .collect();
        // Without an explicit instance, the first file picks the editor
        // whose workspace contains it, like the `token` command.
        let target = match (instance, paths.first()) {
            (Some(id), _) => Target::Instance(id),
            (None, Some(first)) => Target::ForPath(first.path.clone()),
            (None, None) => Target::Default,
        };
        response(target, AutomationRequest::OpenPaths { paths, wait: false }).await
    }

    #[tool(description = "Insert text through Token's real update loop")]
    async fn insert_text(
        &self,
        Parameters(TextParams { text, instance }): Parameters<TextParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::InsertText { text }).await
    }

    #[tool(description = "Set the primary cursor using zero-based line and column coordinates")]
    async fn set_cursor(
        &self,
        Parameters(PositionParams {
            line,
            column,
            instance,
        }): Parameters<PositionParams>,
    ) -> CallToolResult {
        response(
            target(instance),
            AutomationRequest::SetCursor { line, column },
        )
        .await
    }

    #[tool(description = "Set one selection using zero-based anchor and head coordinates")]
    async fn set_selection(
        &self,
        Parameters(SelectionParams {
            anchor_line,
            anchor_column,
            head_line,
            head_column,
            instance,
        }): Parameters<SelectionParams>,
    ) -> CallToolResult {
        response(
            target(instance),
            AutomationRequest::SetSelection {
                anchor_line,
                anchor_column,
                head_line,
                head_column,
            },
        )
        .await
    }

    #[tool(description = "Execute a named keymap action through Token's real update loop")]
    async fn execute_action(
        &self,
        Parameters(ActionParams { name, instance }): Parameters<ActionParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::ExecuteAction { name }).await
    }

    #[tool(
        description = "Insert text and profile snapshot, queue, tree-sitter highlighting, outline, apply, and presentation latency"
    )]
    async fn profile_syntax(
        &self,
        Parameters(TextParams { text, instance }): Parameters<TextParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::ProfileSyntax { text }).await
    }

    #[tool(description = "Scroll the active Token editor by logical lines")]
    async fn scroll(
        &self,
        Parameters(ScrollParams { lines, instance }): Parameters<ScrollParams>,
    ) -> CallToolResult {
        response(target(instance), AutomationRequest::Scroll { lines }).await
    }

    #[tool(
        description = "Render a bounded number of full frames and return stage timing statistics"
    )]
    async fn profile_frames(
        &self,
        Parameters(ProfileParams { frames, instance }): Parameters<ProfileParams>,
    ) -> CallToolResult {
        response(
            target(instance),
            AutomationRequest::ProfileFrames { frames },
        )
        .await
    }

    #[tool(
        description = "Type into the active overlay's input (e.g. the command palette) — the state response's `overlay` field reports the filtered rows and selection"
    )]
    async fn set_overlay_input(
        &self,
        Parameters(TextParams { text, instance }): Parameters<TextParams>,
    ) -> CallToolResult {
        response(
            target(instance),
            AutomationRequest::SetOverlayInput { text },
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for TokenMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "token-editor",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Automate running Token editors without moving the system cursor. Call list_instances to see every open editor window (one per process); every other tool takes an optional `instance` id and defaults to the most recently focused editor. Inspect state first, use semantic cursor, selection, text, and named-action operations for interaction, and profile_frames for bounded real-window renderer measurements.",
            )
    }
}

async fn response(target: Target, request: AutomationRequest) -> CallToolResult {
    let outcome = tokio::task::spawn_blocking(move || {
        automation::request_with_timeout(&target, request, Some(automation::RESPONSE_TIMEOUT))
    })
    .await;
    match outcome {
        Ok(Ok(response)) => match serde_json::to_value(&response) {
            Ok(value) if response.ok => CallToolResult::structured(value),
            Ok(value) => CallToolResult::structured_error(value),
            Err(error) => tool_error(error.to_string()),
        },
        Ok(Err(error)) => tool_error(error.to_string()),
        Err(error) => tool_error(format!("automation task failed: {error}")),
    }
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

pub(crate) fn run() -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(async {
            TokenMcp
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|e| e.to_string())?
                .waiting()
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
}
