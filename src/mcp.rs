//! Stdio MCP server backed by Token's local automation endpoint.

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};
use serde::Deserialize;

use crate::automation::{self, AutomationRequest};

#[derive(Debug, Deserialize, JsonSchema)]
struct TextParams {
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScrollParams {
    lines: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ProfileParams {
    frames: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PositionParams {
    line: usize,
    column: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SelectionParams {
    anchor_line: usize,
    anchor_column: usize,
    head_line: usize,
    head_column: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ActionParams {
    name: String,
}

#[derive(Debug, Clone)]
struct TokenMcp;

#[tool_router]
impl TokenMcp {
    #[tool(
        description = "Inspect the running Token editor and its latest performance measurements"
    )]
    async fn get_state(&self) -> CallToolResult {
        response(AutomationRequest::State).await
    }

    #[tool(description = "Read the active Token document when it is at most 3 MiB")]
    async fn get_document(&self) -> CallToolResult {
        response(AutomationRequest::Document).await
    }

    #[tool(description = "List named actions currently bound in the running Token editor")]
    async fn list_actions(&self) -> CallToolResult {
        response(AutomationRequest::Actions).await
    }

    #[tool(description = "Insert text through Token's real update loop")]
    async fn insert_text(
        &self,
        Parameters(TextParams { text }): Parameters<TextParams>,
    ) -> CallToolResult {
        response(AutomationRequest::InsertText { text }).await
    }

    #[tool(description = "Set the primary cursor using zero-based line and column coordinates")]
    async fn set_cursor(
        &self,
        Parameters(PositionParams { line, column }): Parameters<PositionParams>,
    ) -> CallToolResult {
        response(AutomationRequest::SetCursor { line, column }).await
    }

    #[tool(description = "Set one selection using zero-based anchor and head coordinates")]
    async fn set_selection(
        &self,
        Parameters(SelectionParams {
            anchor_line,
            anchor_column,
            head_line,
            head_column,
        }): Parameters<SelectionParams>,
    ) -> CallToolResult {
        response(AutomationRequest::SetSelection {
            anchor_line,
            anchor_column,
            head_line,
            head_column,
        })
        .await
    }

    #[tool(description = "Execute a named keymap action through Token's real update loop")]
    async fn execute_action(
        &self,
        Parameters(ActionParams { name }): Parameters<ActionParams>,
    ) -> CallToolResult {
        response(AutomationRequest::ExecuteAction { name }).await
    }

    #[tool(
        description = "Insert text and profile snapshot, queue, tree-sitter highlighting, outline, apply, and presentation latency"
    )]
    async fn profile_syntax(
        &self,
        Parameters(TextParams { text }): Parameters<TextParams>,
    ) -> CallToolResult {
        response(AutomationRequest::ProfileSyntax { text }).await
    }

    #[tool(description = "Scroll the active Token editor by logical lines")]
    async fn scroll(
        &self,
        Parameters(ScrollParams { lines }): Parameters<ScrollParams>,
    ) -> CallToolResult {
        response(AutomationRequest::Scroll { lines }).await
    }

    #[tool(
        description = "Render a bounded number of full frames and return stage timing statistics"
    )]
    async fn profile_frames(
        &self,
        Parameters(ProfileParams { frames }): Parameters<ProfileParams>,
    ) -> CallToolResult {
        response(AutomationRequest::ProfileFrames { frames }).await
    }

    #[tool(
        description = "Type into the active overlay's input (e.g. the command palette) — the state response's `overlay` field reports the filtered rows and selection"
    )]
    async fn set_overlay_input(
        &self,
        Parameters(TextParams { text }): Parameters<TextParams>,
    ) -> CallToolResult {
        response(AutomationRequest::SetOverlayInput { text }).await
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
                "Automate a running Token editor without moving the system cursor. Inspect state first, use semantic cursor, selection, text, and named-action operations for interaction, and profile_frames for bounded real-window renderer measurements.",
            )
    }
}

async fn response(request: AutomationRequest) -> CallToolResult {
    match tokio::task::spawn_blocking(move || automation::request(request)).await {
        Ok(Ok(response)) => match serde_json::to_value(&response) {
            Ok(value) if response.ok => CallToolResult::structured(value),
            Ok(value) => CallToolResult::structured_error(value),
            Err(error) => tool_error(error.to_string()),
        },
        Ok(Err(error)) => tool_error(error),
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
