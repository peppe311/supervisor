// SPDX-License-Identifier: MPL-2.0
#![cfg_attr(windows, windows_subsystem = "windows")]

mod agent_runtime;
mod agent_scripts;
mod artifact;
mod audit_runtime;
mod bounded_io;
mod brand;
mod browser;
mod capture_runtime;
mod claude_provider;
mod commands;
mod computer_use_frame;
mod external_browser;
mod file_attachment;
mod file_editor;
mod git_diff;
mod markdown;
mod model_catalog;
mod navigation;
mod permissions;
mod preferences;
mod process_runtime;
mod project_registry;
mod provider;
mod provider_types;
mod remote_desktop;
mod run_start_gate;
mod safety_runtime;
mod sensitive_path;
mod ssh_runtime;
mod subscription_provider;
#[cfg(windows)]
mod system_tray;
mod tab_context;
mod terminal;
mod time_machine;
mod ui_automation;
mod ui_development;
mod window_runtime;
mod workspace;

use anyhow::Context;
use browser::{BrowserApp, BrowserEvent};
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("central_agent=info,wry=warn")),
        )
        .with_target(false)
        .compact()
        .init();

    if std::env::args_os().any(|argument| {
        argument == "--check-native-mcp-requests"
            || argument == "--check-native-command-policy"
            || argument == "--check-native-history"
            || argument == "--check-native-settings"
    }) {
        return browser::check_native_conversation();
    }
    if std::env::args_os().any(|argument| argument == "--check-native-conversation") {
        anyhow::ensure!(
            std::env::args_os().any(|argument| argument == "--allow-test-inference"),
            "Native conversation acceptance requires --allow-test-inference: signed-in Codex subscription usage for two prompts (three with --graph; three turns plus steering with --delivery, optionally --graph; one prompt and an exact print-only command with --approvals, optionally --graph and --decline or --cancel; add --file-change to --approvals to test one exact disposable-file replacement instead; one text/image prompt with --files, optionally --graph; two prompts with --crash, optionally --graph and --lost-ack, forcibly terminating only the test's owned native server and optionally delaying its actual host ACK; two Codex prompts with --providers, optionally --graph, using only local Claude fixtures and no Claude inference; two prompts across two separate app processes with --restart, optionally --graph). Tests use disposable workspaces."
        );
        return browser::check_native_conversation();
    }
    if std::env::args_os().any(|argument| argument == "--check-ui-startup") {
        return browser::check_ui_startup();
    }

    let result = run_application();
    #[cfg(windows)]
    if let Err(error) = &result {
        rfd::MessageDialog::new()
            .set_title("Supervisor")
            .set_level(rfd::MessageLevel::Error)
            .set_description(format!("Supervisor could not start.\n\n{error:#}"))
            .show();
    }
    result
}

fn run_application() -> anyhow::Result<()> {
    #[cfg(windows)]
    let _single_instance = match system_tray::acquire_single_instance()? {
        Some(instance) => instance,
        None => return Ok(()),
    };
    let mut event_loop_builder = EventLoop::<BrowserEvent>::with_user_event();
    let event_loop = event_loop_builder
        .build()
        .context("failed to create the Windows event loop")?;
    let proxy = event_loop.create_proxy();
    let mut app = BrowserApp::new(proxy)?;

    event_loop
        .run_app(&mut app)
        .context("Supervisor closed with an error")?;

    if let Some(error) = app.take_startup_error() {
        return Err(error);
    }

    Ok(())
}
