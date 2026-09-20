# Computer Use speed comparison — 2026-09-16

The user explicitly authorized a live desktop speed test. This compares the
official plugin alone with the same plugin plus Supervisor's companion skill.
It measures native App Server turn execution, not Supervisor's UI submission,
desktop indicator, or application startup.

## Conditions

- Runtime: `0.154.0-alpha.6.2`; official Computer Use plugin: `26.908.70816`.
- Model: `gpt-5.6-luna`; reasoning effort: `low`; speed: Standard
  (`serviceTier: default`); reasoning summaries: `auto`.
- Both variants use fresh ephemeral threads in Supervisor's actual native
  profile. An independent hidden Windows process verifies physical file paths
  before starting; credentials and databases are not copied.
- The official skill is attached to both. Only the optimized variant attaches
  `supervisor-computer-use`. A per-thread `skills.config` override disables the
  companion in the baseline; shared preferences and the vendor plugin are not
  edited. No test tasks are retained in the chat list.
- Both use the same Full access setting for the ephemeral thread. The prompt
  limits operations to Calculator and reading installed skill documentation.
  The harness declines any approval request and stops that attempt.
- Calculator starts in Standard mode showing `0`. The same prompt asks the
  agent to clear it, calculate `7 + 8` through the interface, verify the display,
  and return `VERIFIED: 15`. It does not supply the expected answer to the agent.
- The timer runs from sending `turn/start` to receiving `turn/completed`.
  App Server connection/thread creation, manual fixture preparation and the
  independent final screenshot check are outside this interval. Reading plugin
  documentation, model responses and desktop operations are inside it.
- Counts and tool durations come from native item events. Tool time is the sum
  of native durations; the remainder includes model and service/transport
  waits and is not a measurement of reasoning alone. No private reasoning or
  screenshot payloads are saved by the harness.

The first pair ran baseline then optimized. A second pair reverses that order
because the first optimized run contained a long individual tool call. The
companion and task prompt remain unchanged between all measured runs.

One preliminary attempt with a Read-only/untrusted test profile stopped at a
documentation-command approval before desktop operation. It was interrupted,
granted no approval, and is excluded from the comparison.

## Results

| Pair | Variant | Total | Tool time | Tool calls | Failed calls | Result |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 1 | Official plugin | 88.952 s | 30.014 s | 12 | 1 | Display independently verified: 15 |
| 1 | With Supervisor companion | 102.344 s | 42.293 s | 11 | 0 | Display independently verified: 15 |
| 2 (first) | With Supervisor companion | 79.279 s | 23.754 s | 10 | 1 | Display independently verified: 15 |
| 2 (second) | Official plugin | 74.434 s | 23.671 s | 9 | 0 | Display independently verified: 15 |

| Aggregate (two runs each) | Official plugin | With Supervisor companion |
| --- | ---: | ---: |
| Mean total time | 81.693 s | 90.812 s |
| Mean tool time | 26.843 s | 33.024 s |
| Mean time outside tools | 54.851 s | 57.788 s |
| Tool calls, both runs | 21 | 21 |
| Failed tool calls, both runs | 1 | 1 |
| Correct displayed result | 2/2 | 2/2 |

**No speed improvement was demonstrated.** The companion variant's mean is
9.119 seconds (11.16%) slower in this small sample. Both variants recovered
from one failed tool invocation and completed both calculations correctly.
The original baseline did not invoke or read the companion in either run.
`turn/start` acknowledgments took 14–17 ms, so the large measured delays here
are after native submission, not acknowledgment latency. This does not measure
the Supervisor composer's own path before `turn/start`.

In the first pair the companion run is 13.392 seconds (15.06%) slower. Its last
desktop call took 15.606 seconds; most preceding observation/action calls took
about 3.1 seconds. It is not valid to subtract the slow call and present a
hypothetical faster result. Fewer calls and no failed call in this run are
observations, not proof of an overall speed or reliability improvement.

The reversed pair took 74.434 seconds for the original and 79.279 seconds with
the companion (6.51% slower). The slower mean is therefore not solely an effect
of including the long call from the first pair. These measurements do not
justify advertising the current companion as a proven speed optimization.

These are repeated trials of one small task on one computer, with no control
over service load, warm caches, or the model's choice of actions. Two samples
per variant cannot establish a statistically reliable or general improvement.
Changing the prompt or companion to select only faster runs would invalidate
this comparison.

## Reproduction

`crates/central-agent-codex-runtime/examples/computer_use_speed_probe.rs` is an
explicit opt-in probe, separate from normal tests. Build it through the repository
build-storage helper. Run it serially in the interactive Windows user's actual
profile with `--allow-test-inference`, the variant (`baseline` or `optimized`),
the existing Supervisor Codex home, both existing skill paths, and a temporary
JSON report path. Restore and inspect Calculator between each run using the
official Computer Use skill. Do not run it unattended or simultaneously with
other desktop actions. It stops on an approval request and bounds each model
turn to 240 seconds.

Raw measurement reports and temporary launch scripts are only needed during
the test and must be removed after recording the results here. The canonical
Supervisor executable needs no rebuild for this measurement-only task.

## Follow-up: isolate observation and input latency

On 2026-09-16 at 00:54 Europe/Rome (22:54 UTC on September 15), the user
authorized a component-level investigation. These measurements use the same
installed official plugin through `@oai/sky` in the existing, warm `node_repl`
session. They do not start additional account-backed model turns or measure
Supervisor's UI, native thread startup, or model inference.

Each sample times the awaited public `sky` method with `performance.now()`.
The action and its immediate screenshot refresh are timed separately. One
observed action is performed per cell, then the refreshed image is inspected
before choosing the next action. No fixed sleeps, private protocol calls,
vendor patches, saved images or alternate Windows automation are used.
Observation time includes the official service/bridge and any screenshot
delivery performed by the SDK; it is not a measurement of the Windows capture
API in isolation.

### Measurements

| Operation | Samples | Mean | Range | Observed result |
| --- | ---: | ---: | ---: | --- |
| Key input with verified visible effect | 3 | 40.31 ms | 39.82–40.62 ms | `Escape`, `7`, `Escape` all changed the display as expected |
| Click with verified visible effect | 3 | 79.53 ms | 75.18–83.47 ms | Clear/digit buttons worked |
| Screenshot refresh after those actions | 6 | 3,054.40 ms | 3,040.34–3,066.83 ms | Each result verified visually |
| Screenshot only, without preceding input | 3 | 4,411.87 ms | 3,080.25–5,079.38 ms | Correct image; later samples took about 5.08 s |
| Accessibility text only, without preceding input | 3 | 3,034.66 ms | 3,030.98–3,036.96 ms | `accessibility: null` in all samples |
| Screenshot and accessibility, without preceding input | 3 | 5,082.01 ms | 5,078.44–5,085.18 ms | Correct image, but `accessibility: null` |

The observation modes were interleaved in this order: screenshot/text/both,
both/screenshot/text, text/both/screenshot. The setup calls, outside the repeated
samples, took 187.32 ms for `list_apps`, 19.34 ms for `get_window`, and
155.89 ms for activation. The single setup samples are not statistical estimates.

For reproducibility, the repeated observation samples in milliseconds were:

- Screenshot only: 3080.2452, 5075.9956, 5079.3835.
- Accessibility only: 3036.0439, 3030.9793, 3036.9631.
- Both: 5078.4435, 5082.4059, 5085.1754.
- Verified keys: 40.6153, 39.8171, 40.4956.
- Verified clicks: 79.9448, 83.4733, 75.1837.
- Refresh after verified input: 3040.3440, 3053.5008, 3065.5186, 3066.8292,
  3050.7487, 3049.4880.

Two diagnostic limitations were observed and are not silently discarded:

- `KP_7` returned successfully in 46.67 ms but left the displayed value at 0.
  A fresh observation preceded use of the regular `7` key, which worked.
  The keypad sample is excluded from the **verified-effect** key summary,
  not characterized as a successful input. No keyboard/system setting was
  changed and the cause of its absent effect was not established.
- A proposed control request with both observation flags false was rejected
  by the public SDK (`get_window_state must request include_text,
  include_screenshot, or both`). It was not retried or treated as a latency
  sample. Window selection and a valid screenshot were refreshed before input.

Calculator was left at 0 after a final verified reset. No other application
was operated, and no production profile or application code was changed.

### Interpretation and limits

The slow portion of the **tool cycle** is the observation call: approximately
3.05 seconds after input versus 0.04 seconds to send a verified key or
0.08 seconds to send a verified click. Repeated observations without input
can cost around 5 seconds. This is compatible with the approximately 3.1-second
action/refresh calls seen in the earlier native benchmark.

It does not establish why the earlier single call took 15.606 seconds; that
long call was not reproduced here. It also does not isolate the internal reason
for the 3/5-second waits. Both image and text observation paths incurred latency.
Read-only inspection of the installed Windows SDK shows the public state method
forwarding to the official service; its public input has no capture-wait setting.
No undocumented timeout, permission or transport behavior was modified.

Avoiding a **duplicate** observation is the supported place to seek savings.
The then-current companion already asked for this, but the prior A/B test did not
demonstrate fewer aggregate calls. Simply disabling accessibility does not
remove the measured 3-second wait: screenshot-only refreshes still incurred it.
Conversely, accessibility-only observation cannot replace screenshots for this
Calculator session because it returned no usable state. Required verification
after an action must remain; old screenshots or indexes must not be reused
after state changes to obtain an artificial speedup.

Finally, these local timings omit model/service waits between calls. In the
earlier baseline those waits averaged 54.851 seconds per task, so improving
observation latency alone would not remove all of the total task latency.

The subsequent [official-component audit and proposed changes](CODEX_COMPUTER_USE_OFFICIAL_AUDIT.md)
documents the inspected SDK/desktop boundary, supported controls, connection
limitations and the subsequently authorized implementation. It does not
establish a native cause for the observed waits.

## Follow-up after retiring the companion

The opt-in probe now supports an explicit public-method timing helper and a
second, read-only Supervisor settings-search scenario. Both variants keep
Luna / low / Standard. The retired companion is a benchmark fixture, not a
production skill. These measurements include the same additional timing
instructions in both variants and are not directly comparable with the earlier
uninstrumented end-to-end times.

A first instrumentation pilot completed Calculator correctly in 66.830 seconds
with eight tool calls and one recovered failure. An independent observation
confirmed 15. Only one timing sample was collected because adjacent output
records lacked guaranteed line separators. This pilot is excluded from the
comparison. The helper now emits explicit line boundaries and the probe rejects
instrumented trials without both successful input and observation samples.
The pilot's result is not presented as a speed improvement.

### Instrumented Calculator trials — 2026-09-16

Both runs requested Luna / low / Standard with the same task, helper and
already-open Calculator reset to 0. The native settings event was absent;
the profile above is the requested profile, not an independently reported one.
Run order was official baseline, then retired companion. Both final displays
were independently observed as 15. These are one trial per variant, not a
statistically reliable performance comparison.

| Variant | Wall time | Tool calls | Failed tools | Input samples | Observation samples | Empty observations | Disposition |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Official baseline | 69.161 s | 8 | 0 | 3 | 6 | 2 | Completed; recorded observation cycle passed |
| Retired companion fixture | 152.661 s | 12 | 1 | 14 | 19 | 15 | Correct final value, but excluded from compliant performance comparisons |

Baseline public input calls averaged 38.31 ms; observations averaged 3045.99 ms.
The tool-duration sum was 19.531 s, leaving 49.630 s outside those tool spans.
The companion's public inputs averaged 63.36 ms and observations 3034.18 ms;
its tool-duration sum was 60.425 s. These means describe recorded public calls,
not verified input effects or private capture stages. A successful input return
alone does not demonstrate that the application changed.

The companion sequence repeatedly placed another input after a text-only
observation with neither accessibility content nor an image. Its original
completion/verdict gate passed, but reviewing the samples exposed the missing
usable-state requirement. The probe now also requires a contiguous recorded
sequence, usable observation before each input, and usable observation after
the final input. Empty observations do not satisfy that requirement. It cannot
prove that the model inspected the returned state, or validate uninstrumented
operations; independent outcome review remains required.

The retired variant was not extended to the second application after this
protocol failure. No acceleration percentage is inferred from these trials.
Retirement is supported as a simplification, not as proof of faster capture.
The official component files, app permissions and observation guarantees remain
unchanged. A standalone reset after the trials was observed at 0. During that
reset an Escape request left 15 unchanged; a screenshot-ID click was rejected
before input. A new observation followed by a coordinate click on the observed
Clear button succeeded. Those setup/cleanup operations are outside the timed runs.

### Second application: Supervisor settings search

The official-only native trial ended after 35.576 s with five tool calls, two
failed tools and no verified result. It produced usable accessibility text in
16.22 ms, then its first recorded click failed in 621.77 ms. It failed the new
recorded-cycle gate and is not a completed speed sample. The privacy-limited
report intentionally retained no arbitrary tool error or final-response text;
the precise cause of that click failure was not established. No repeated model
attempt or companion variant was run in this application.

A separate, untimed public-SDK smoke check refreshed Supervisor's state, clicked
the observed Search settings field, entered `Reasoning summaries`, observed the
single matching result, and cleared the search using its observed Clear button.
Each successful action was followed by a new state. A `set_value` attempt during
cleanup failed while reading the UIA read-only property (`CacheRequest`,
`0x80070057`); a fresh observation confirmed the text remained before the Clear
button was selected. This is a distinct observed accessibility limitation, not
a proven explanation of the native trial's earlier click failure.

The search and return to workspace completed without changing any preference
or sending a chat prompt. This smoke check demonstrates the exercised public
operations in this session; it is not a substitute for a passing Luna-driven
trial. Calculator's approximately three-second observation cost must not be
generalized to every application: the Supervisor text response above was much
faster, although a single response is not a representative latency estimate.

Remaining work for a future performance change: diagnose the failed native
selection with bounded error categories, obtain compliant repeated trials on
both apps, and balance variant ordering. This implementation makes no general
speed claim and does not modify the official SDK to hide these failures.
