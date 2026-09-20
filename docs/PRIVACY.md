# Data and permissions

Supervisor stores local application state in the user's application-data
directory, using the historical `CentralAgent` name for compatibility. This can
include conversations, attachments, settings, browser profiles, provider state
and diagnostic/audit records. It is separate from this source repository.

Prompts, selected files, screenshots, tool results and other supplied context
can be sent to the configured model provider. Websites opened in the browser
receive normal browser traffic. Plugins and remote SSH services communicate
with their own endpoints and have their own terms and retention policies.
Local storage does not mean model processing is offline.

Provider credentials are managed through the provider connection flow. Never
commit or share credential stores, database copies, complete logs or captured
browser sessions. Review any attachment before sending it. Secret-file checks
and redacted diagnostics reduce accidental exposure; they cannot identify every
private value in arbitrary content.

Codex sandbox/approval settings apply to Codex's native tools. Supervisor's tool
confirmations apply to its own browser, terminal and SSH capabilities. Neither
UI setting turns the whole application into an OS sandbox. Use the narrowest
access appropriate to the project and review persistent full-access choices.

Closing the window can keep agents running in the Windows tray. Stop the turn or
exit from the tray to interrupt work. Preserve account and conversation data
when troubleshooting; never copy another application's native database or
credentials to work around a profile mismatch.
