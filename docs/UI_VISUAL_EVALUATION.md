# UI visual evaluation

Central Agent evaluates product surfaces against stable operator tasks rather
than isolated component beauty shots. The versioned scenario manifest is
`ui/evals/scenarios.json`.

## What a review run records

For every affected scenario, retain:

- scenario ID and setup;
- Git revision or working-tree label;
- model and prompt when an agent produced the change;
- `DESIGN.md` revision;
- theme and viewport profile;
- first useful frame and full-surface capture;
- keyboard, scrolling, reflow, and reduced-motion observations;
- concise reviewer findings and disposition.

Do not place credentials, private page content, terminal secrets, user files, or
remote desktop data in committed fixtures. Use deterministic synthetic content.

## Comparison method

Use matched conditions when comparing two implementations: identical scenario,
fixture data, theme, viewport, zoom, and application state. Randomize or hide
the implementation label when asking for a preference review.

Review in this order:

1. Can the operator identify the current surface, target, and state?
2. Is the task's primary object dominant and usable?
3. Does each region own one clear responsibility?
4. Are controls readable, reachable, and stable while content changes?
5. Are system actions, approvals, errors, and remote targets honest?
6. Do Light and Dark preserve the same hierarchy and contrast?
7. Does the layout remain coherent in Full HD, 2K, and 4K?
8. Is anything decorative, duplicated, too small, or unnecessarily enclosed?
9. Is every trusted product surface limited to white, black, and neutral gray?
10. Do controls keep four equivalent corners and follow their surface contract, including borderless Settings controls?
11. Is the Central Agent brand mark absent from operational UI while functional glyphs remain clear?

## Routing corrections

Put a correction in the narrowest durable layer:

- product judgment or recurring composition failure: `DESIGN.md`;
- shared visual value: `assets/themes.css`;
- reusable structure or behavior: a Svelte component;
- mechanically detectable regression: `scripts/verify-design-contract.mjs`;
- new operator task or materially different state: the scenario manifest;
- one-off content or fixture failure: the fixture, not the design contract.

Human review remains the release decision. Static checks prevent known classes
of regression; they do not score taste or prove that a surface is finished.
