<div class="workspace-explorer-head">
  <span class="workspace-explorer-title">Projects</span>
  <div class="workspace-explorer-head-actions">
    <button
      id="workspace-project-search-toggle"
      class="workspace-explorer-action"
      type="button"
      title="Search projects and chats"
      aria-label="Search projects and chats"
      aria-expanded="false">
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="8.75" cy="8.75" r="4.75" />
        <path d="m12.25 12.25 4 4" />
      </svg>
    </button>
    <button
      id="workspace-explorer-add"
      class="workspace-explorer-action workspace-explorer-add"
      type="button"
      title="Add project"
      aria-label="Add project">
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <path d="M2.75 6.25h5l1.45 1.6h8.05v8.4H2.75zM14.25 2.75v3.5M12.5 4.5H16" />
      </svg>
    </button>
  </div>
</div>
<div id="workspace-project-search" class="workspace-project-search" hidden>
  <svg viewBox="0 0 20 20" aria-hidden="true">
    <circle cx="8.75" cy="8.75" r="4.75" />
    <path d="m12.25 12.25 4 4" />
  </svg>
  <input
    id="workspace-project-search-input"
    type="search"
    maxlength="120"
    placeholder="Search projects and chats"
    autocomplete="off"
    spellcheck="false" />
  <button id="workspace-project-search-close" type="button" aria-label="Close search">×</button>
</div>
<div id="workspace-explorer-projects" class="workspace-explorer-projects" aria-label="Supervisor projects" hidden></div>
<div id="workspace-explorer-root" class="workspace-explorer-root" hidden>
  <div class="workspace-files-section-head">
    <span>Files</span>
    <div class="workspace-explorer-file-actions" aria-label="Active project file actions">
      <button
        id="workspace-explorer-new-file"
        class="workspace-explorer-action"
        type="button"
        title="New file"
        aria-label="Create a file in the selected folder"
        disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M5.5 2.75h5.4l3.6 3.6v10.9h-9zM10.75 2.75v3.9h3.75M7.7 11h4.6M10 8.7v4.6" />
        </svg>
      </button>
      <button
        id="workspace-explorer-new-folder"
        class="workspace-explorer-action"
        type="button"
        title="New folder"
        aria-label="Create a folder in the selected folder"
        disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M2.75 5.25h5l1.45 1.6h8.05v9.4H2.75zM12.5 9.3v4.5M10.25 11.55h4.5" />
        </svg>
      </button>
      <button
        id="workspace-explorer-import"
        class="workspace-explorer-action"
        type="button"
        title="Import files"
        aria-label="Import existing files into the selected folder"
        disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M10 2.75v9.2M6.8 8.8 10 12l3.2-3.2M3.25 13.25v4h13.5v-4" />
        </svg>
      </button>
      <button
        id="workspace-explorer-refresh"
        class="workspace-explorer-action"
        type="button"
        title="Refresh active project"
        aria-label="Refresh active project"
        disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M15.7 7A6.25 6.25 0 1 0 16 11M15.7 3.5V7h-3.5" />
        </svg>
      </button>
    </div>
  </div>
  <div class="workspace-explorer-file-head">
    <button
      id="workspace-explorer-root-select"
      class="workspace-explorer-root-line selected"
      type="button"
      aria-label="Use the project root for new files">
      <span id="workspace-explorer-root-visual" class="workspace-explorer-root-visual" aria-hidden="true">
        <span id="workspace-explorer-root-icon" class="workspace-root-icon">
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path d="M2.75 5.5h5l1.45 1.6h8.05v8.15H2.75z" />
          </svg>
        </span>
      </span>
      <span class="workspace-explorer-root-copy">
        <span id="workspace-explorer-name" class="workspace-explorer-name"></span>
        <span id="workspace-explorer-path" class="workspace-explorer-path"></span>
      </span>
    </button>
  </div>
  <form id="workspace-explorer-create" class="workspace-explorer-create" hidden>
    <div class="workspace-explorer-create-context">
      <label id="workspace-explorer-create-label" for="workspace-explorer-create-input">New file</label>
      <span id="workspace-explorer-create-target">Project root</span>
    </div>
    <input
      id="workspace-explorer-create-input"
      type="text"
      maxlength="240"
      autocomplete="off"
      spellcheck="false"
      aria-describedby="workspace-explorer-create-target" />
    <div class="workspace-explorer-create-actions">
      <button id="workspace-explorer-create-confirm" type="submit">Create</button>
      <button id="workspace-explorer-create-cancel" type="button">Cancel</button>
    </div>
  </form>
</div>
<div id="workspace-explorer-tree" class="workspace-explorer-tree"></div>
<div id="workspace-explorer-empty" class="workspace-explorer-empty">
  <span id="workspace-explorer-empty-copy">Add a project to browse and work with its files.</span>
  <button id="workspace-explorer-select" class="checkpoint-action" type="button">Add project</button>
</div>
<div id="workspace-explorer-foot" class="workspace-explorer-foot" hidden></div>

<div id="workspace-remove-project-dialog" class="workspace-confirm-backdrop" hidden>
  <div
    class="workspace-confirm-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="workspace-remove-project-title"
    aria-describedby="workspace-remove-project-copy workspace-remove-project-warning">
    <h2 id="workspace-remove-project-title">Remove project?</h2>
    <p id="workspace-remove-project-copy">
      <strong id="workspace-remove-project-name">This project</strong><span id="workspace-remove-project-detail"> and its local chats will be removed from Supervisor. Files in the project folder will not be changed.</span>
    </p>
    <p id="workspace-remove-project-warning" class="workspace-confirm-warning" hidden></p>
    <div class="workspace-confirm-actions">
      <button id="workspace-remove-project-cancel" type="button">Cancel</button>
      <button id="workspace-remove-project-confirm" class="workspace-confirm-remove" type="button">Remove project</button>
    </div>
  </div>
</div>

<div id="workspace-delete-chat-dialog" class="workspace-confirm-backdrop" hidden>
  <div
    class="workspace-confirm-dialog"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="workspace-delete-chat-title"
    aria-describedby="workspace-delete-chat-copy">
    <h2 id="workspace-delete-chat-title">Remove local chat?</h2>
    <p id="workspace-delete-chat-copy">
      <strong id="workspace-delete-chat-name">New chat</strong><span> will be permanently removed from this project's local history. Checkpoints and project files will not be deleted. This local removal cannot be undone.</span>
    </p>
    <div class="workspace-confirm-actions">
      <button id="workspace-delete-chat-cancel" type="button">Cancel</button>
      <button id="workspace-delete-chat-confirm" class="workspace-confirm-remove" type="button">Remove local chat</button>
    </div>
  </div>
</div>

<div id="workspace-move-chat-dialog" class="workspace-confirm-backdrop" hidden>
  <div
    class="workspace-confirm-dialog workspace-move-chat-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="workspace-move-chat-title"
    aria-describedby="workspace-move-chat-copy">
    <h2 id="workspace-move-chat-title">Change project</h2>
    <p id="workspace-move-chat-copy">
      <strong id="workspace-move-chat-name">New chat</strong><span> will keep its complete conversation history. New agent runs and Explorer will use the project you choose.</span>
    </p>
    <div
      id="workspace-move-chat-projects"
      class="workspace-move-chat-projects"
      role="list"
      aria-label="Available destination projects"></div>
    <p id="workspace-move-chat-empty" class="workspace-move-chat-empty" hidden>
      No other project is connected yet. Choose another folder below.
    </p>
    <button id="workspace-move-chat-browse" class="workspace-move-chat-project workspace-move-chat-browse" type="button">
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <path d="M2.75 6.25h5l1.45 1.6h8.05v8.4H2.75zM14.25 2.75v3.5M12.5 4.5H16" />
      </svg>
      <span class="workspace-move-chat-project-copy">
        <span class="workspace-move-chat-project-name">Choose another folder…</span>
        <span class="workspace-move-chat-project-path">Add it as a project and keep this chat</span>
      </span>
    </button>
    <div class="workspace-confirm-actions">
      <button id="workspace-move-chat-cancel" type="button">Cancel</button>
    </div>
  </div>
</div>
