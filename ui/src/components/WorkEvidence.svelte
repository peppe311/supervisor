<script lang="ts">
  import {commandOutcome,workStatus,type WorkReport} from '../lib/work-results';
  let {report}:{report:WorkReport}=$props();
  const failed=$derived(report.commands.filter(command=>command.status==='failed').length);
  const relevant=$derived(report.evidence.events.filter(event=>event.role==='system'||event.kind==='activity'||event.phase==='commentary'||event.attachments.length));
</script>

<article class="evidence" aria-label={`Work evidence for ${report.title}`} data-work-evidence={report.owner}>
  <div class="evidence-heading"><h3>{report.title}</h3><span>{workStatus(report)}</span></div>
  <p class="scope">Latest recorded turn{report.partial?' · Details not fully loaded':''}</p>
  {#if report.historyError}<p role="alert">{report.historyError}</p>{:else if report.historyState==='loading'}<p class="scope" role="status">Loading work details…</p>{/if}
  {#if !report.loaded}<p>No recorded work is loaded. Open the conversation or refresh its history.</p>{/if}
  {#if report.evidence.objective}<div><h4>Request</h4><p class="prose">{report.evidence.objective}</p></div>{/if}
  <div class="totals">
    {#if report.diff}<span><b>+{report.diff.additions} −{report.diff.deletions}</b> · {report.diff.fileCount} files</span>{:else}<span>Diff not reported</span>{/if}
    <span>{report.commands.length} recorded commands{failed?` · ${failed} failed`:''}</span>
    {#if report.pendingRequests}<span>{report.pendingRequests} pending requests</span>{/if}
  </div>
  {#if report.assessment}<div><h4>Supervisor assessment</h4><p class="prose">{report.assessment.summary}</p></div>{/if}
  {#if report.evidence.finalAnswer}<details open><summary>Reported result</summary><p class="prose">{report.evidence.finalAnswer}</p></details>{/if}
  {#if report.commands.length}<details><summary>Commands & checks</summary>
    <p class="scope">Exit codes are reported by the runtime. They do not certify all acceptance criteria.</p>
    {#each report.commands as command}<div class="record"><div class="record-head"><code>{command.command}</code><span>{commandOutcome(command)}</span></div>{#if command.output}<pre>{command.output}</pre>{/if}</div>{/each}
  </details>{:else if report.loaded}<p class="scope">No command or test outcome is reported in the available details.</p>{/if}
  {#if report.files.length}<details><summary>Changed files · {report.files.length}</summary><ul>{#each report.files as file}<li><code>{file.path}</code><span>{file.state}</span></li>{/each}</ul></details>{/if}
  {#if report.checks.length}<details><summary>Reported plan</summary><ul>{#each report.checks as check}<li><span>{check.step}</span><span>{check.status}</span></li>{/each}</ul></details>{/if}
  {#if relevant.length}<details><summary>Activity, decisions & attachments</summary>
    {#each relevant as event}<div class="record"><div class="record-head"><strong>{event.text||event.category||'Activity'}</strong>{#if event.status}<span>{event.status}</span>{/if}</div>
      {#if event.detail}<pre>{event.detail}</pre>{/if}
      {#if event.diff}<details><summary>Recorded diff</summary><pre>{event.diff}</pre></details>{/if}
      {#each event.attachments as file}<p class="scope">{file.name} · {file.kind} · in source history</p>{/each}
    </div>{/each}
  </details>{/if}
  {#if report.evidence.eventsOmitted||report.commandsOmitted||report.filesTruncated}<p class="scope">This bounded summary omits some older or lengthy evidence. The original conversation retains its history.</p>{/if}
</article>

<style>
  .evidence{display:grid;align-content:start;gap:var(--ca-space-4);min-width:0;padding:var(--ca-space-4);border-radius:var(--ca-radius-large);background:var(--ca-surface-2);color:var(--ca-text)}
  h3,h4,p{margin:0}h3{font:650 var(--ca-type-title)/var(--ca-leading-compact) var(--ca-font-body)}h4{margin-bottom:var(--ca-space-2);font-size:var(--ca-type-label);font-weight:600}
  .evidence-heading,.record-head{display:flex;justify-content:space-between;align-items:baseline;gap:var(--ca-space-3)}.evidence-heading span,.record-head>span{flex:none;color:var(--ca-muted);font-size:var(--ca-type-caption)}
  .scope{color:var(--ca-muted);font-size:var(--ca-type-caption);line-height:var(--ca-leading-body)}.prose{white-space:pre-wrap;overflow-wrap:anywhere;font-size:var(--ca-type-label);line-height:var(--ca-leading-body)}
  .totals{display:flex;flex-wrap:wrap;gap:var(--ca-space-2) var(--ca-space-4);font-size:var(--ca-type-caption);color:var(--ca-muted)}.totals b{color:var(--ca-text)}
  details{min-width:0}summary{padding-block:var(--ca-space-2);cursor:pointer;font-size:var(--ca-type-label);font-weight:600}summary:hover{color:var(--ca-muted)}summary:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}
  .record{display:grid;gap:var(--ca-space-2);padding-block:var(--ca-space-3)}code,pre{font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-mono);white-space:pre-wrap;overflow-wrap:anywhere;min-width:0;margin:0}pre{max-height:240px;overflow:auto;color:var(--ca-muted)}.record-head strong{font-size:var(--ca-type-label);font-weight:500;overflow-wrap:anywhere}
  ul{list-style:none;padding:0;margin:0;display:grid;gap:var(--ca-space-2)}li{display:flex;justify-content:space-between;gap:var(--ca-space-3);font-size:var(--ca-type-label)}li>span:last-child{color:var(--ca-muted);font-size:var(--ca-type-caption)}
</style>
