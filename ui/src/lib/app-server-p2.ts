export type CommandAccess = "readOnly" | "workspaceWrite";
export type NudgeCreditType = "credits" | "usage_limit";

export type P2Action =
  | { kind: "sync_scope" }
  | { kind: "refresh_features" }
  | { kind: "set_feature"; view_id: string; name: string; enabled: boolean }
  | { kind: "detect_import"; scope_id: string; include_home: boolean; include_project: boolean }
  | { kind: "import"; view_id: string; item_ids: string[] }
  | { kind: "read_import_history" }
  | { kind: "consume_reset" }
  | { kind: "retry_reset"; attempt_id: string }
  | { kind: "send_nudge"; credit_type: NudgeCreditType }
  | { kind: "submit_feedback"; classification: "bug" | "bad_result" | "suggestion" | "other"; reason: string }
  | { kind: "run_command"; scope_id: string; command: string[]; access: CommandAccess; rows: number; cols: number }
  | { kind: "write_command"; process_id: string; input: string; close_stdin: boolean }
  | { kind: "resize_command"; process_id: string; rows: number; cols: number }
  | { kind: "terminate_command"; process_id: string }
  | { kind: "clear_command"; process_id: string };

export interface AppServerP2View {
  scope: { id: string | null; project: string | null };
  features: {
    viewId: string | null;
    loading: boolean;
    loaded: boolean;
    current: boolean;
    entries: Array<{
      name: string;
      stage: "beta" | "underDevelopment" | "stable" | "deprecated" | "removed";
      displayName: string | null;
      description: string | null;
      announcement: string | null;
      enabled: boolean;
      defaultEnabled: boolean;
      mutable: boolean;
    }>;
    error: string | null;
    notice: string | null;
  };
  migration: {
    viewId: string | null;
    busy: boolean;
    current: boolean;
    items: Array<{ id: string; itemType: string; description: string; scope: string }>;
    connectors: Array<{ name: string; source: string; sessionCount: number }>;
    activeImport: { status: string; results: Array<{ itemType: string; successes: number; failures: number }> } | null;
    histories: Array<{ completedAtMs: string; provider: boolean; successes: number; failures: number }>;
    error: string | null;
    notice: string | null;
  };
  accountActions: {
    busy: boolean;
    resetRetryId: string | null;
    resetStatus: string | null;
    nudgeStatus: string | null;
    error: string | null;
  };
  feedback: { allowed: boolean; busy: boolean; status: string | null; error: string | null };
  command: {
    process: {
      id: string;
      project: string;
      argv: string[];
      access: CommandAccess;
      status: string;
      exitCode: number | null;
      output: Array<{ stream: "stdout" | "stderr"; text: string }>;
      truncated: boolean;
    } | null;
    controlBusy: boolean;
    error: string | null;
    notice: string | null;
  };
}

export function emptyAppServerP2(): AppServerP2View {
  return {
    scope: { id: null, project: null },
    features: { viewId: null, loading: false, loaded: false, current: false, entries: [], error: null, notice: null },
    migration: { viewId: null, busy: false, current: false, items: [], connectors: [], activeImport: null, histories: [], error: null, notice: null },
    accountActions: { busy: false, resetRetryId: null, resetStatus: null, nudgeStatus: null, error: null },
    feedback: { allowed: false, busy: false, status: null, error: null },
    command: { process: null, controlBusy: false, error: null, notice: null }
  };
}
