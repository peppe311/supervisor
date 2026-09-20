//! Shared immutable snapshot projection for accepted provider input.
use super::*;

pub(super) fn tabs(tabs: &[TabContextSnapshot]) -> Vec<AgentTabContext> {
    tabs.iter().map(tab).collect()
}

pub(super) fn tab(snapshot: &TabContextSnapshot) -> AgentTabContext {
    AgentTabContext {
        tab_id: snapshot.tab_id,
        title: snapshot.title.clone(),
        url: snapshot.url.clone(),
        description: snapshot.description.clone(),
        language: snapshot.language.clone(),
        text: snapshot.text.clone(),
        source_text_char_count: snapshot.text_char_count,
        elements: snapshot
            .elements
            .iter()
            .map(|element| AgentContextElement {
                role: element.role.clone(),
                name: element.name.clone(),
                disabled: element.disabled,
            })
            .collect(),
        source_element_count: snapshot.source_element_count,
        links: snapshot
            .links
            .iter()
            .map(|link| AgentContextLink {
                text: link.text.clone(),
                url: link.url.clone(),
            })
            .collect(),
        source_link_count: snapshot.source_link_count,
        images: snapshot
            .images
            .iter()
            .map(|image| AgentContextImage {
                alt: image.alt.clone(),
                url: image.url.clone(),
                kind: image.kind.clone(),
                width: image.width,
                height: image.height,
            })
            .collect(),
        source_image_count: snapshot.source_image_count,
        // Remaining adapters receive text/metadata, not raw screenshots.
        visual_included: false,
        estimated_token_count: snapshot.estimated_token_count,
        truncated: snapshot.truncated,
        redaction_count: snapshot.redaction_count,
    }
}

pub(super) fn terminals(terminals: &[DraftTerminalContext]) -> Vec<AgentTerminalContext> {
    terminals.iter().map(terminal).collect()
}

pub(super) fn terminal(context: &DraftTerminalContext) -> AgentTerminalContext {
    AgentTerminalContext {
        session_id: context.snapshot.session_id,
        label: context.snapshot.label.clone(),
        kind: terminal_kind_name(context.snapshot.kind).to_owned(),
        profile_id: context.snapshot.profile_id.clone(),
        shell: if context.snapshot.kind == TerminalKind::Ssh {
            "OpenSSH".to_owned()
        } else {
            context.snapshot.shell.clone()
        },
        cwd: context.snapshot.cwd.clone(),
        status: context.snapshot.status.clone(),
        phase: terminal_phase_name(context.snapshot.phase).to_owned(),
        busy: context.snapshot.busy,
        output: context.snapshot.output.clone(),
        source_output_char_count: context.snapshot.source_output_char_count,
        output_revision: context.snapshot.output_revision,
        last_exit_code: context.snapshot.last_exit_code,
        captured_at_ms: context.snapshot.captured_at_ms,
        estimated_token_count: context.snapshot.estimated_token_count,
        redaction_count: context.snapshot.redaction_count,
        truncated: context.snapshot.truncated,
        followed_live: context.follow_live,
    }
}
