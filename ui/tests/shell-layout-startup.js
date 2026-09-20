// Runs in disposable startup-check WebViews. It inspects only synthetic shell
// geometry and styles; it does not use Computer Use or production browser data.
try {
  const closeEnough = (left, right) => Math.abs(left - right) < 0.5;
  const verifyToolbar = () => {
    const address = document.querySelector('.address-shell');
    const agent = document.getElementById('toggle-agent');
    const terminal = document.getElementById('toggle-terminal');
    const addressBounds = address.getBoundingClientRect();
    const agentBounds = agent.getBoundingClientRect();
    const terminalBounds = terminal.getBoundingClientRect();
    if (!closeEnough(agentBounds.height, addressBounds.height)
        || !closeEnough(terminalBounds.height, addressBounds.height)) {
      throw new Error(`Workspace controls do not share the address height: ${JSON.stringify({address:addressBounds.height,agent:agentBounds.height,terminal:terminalBounds.height})}`);
    }
    if (!closeEnough(agentBounds.width, terminalBounds.width)) {
      throw new Error('Agent and Terminal no longer have equal outer widths');
    }
    for (const pressed of ['false', 'true']) {
      agent.setAttribute('aria-pressed', pressed);
      const style = getComputedStyle(agent);
      if (['Top','Right','Bottom','Left'].some(side => parseFloat(style[`border${side}Width`]) !== 0)) {
        throw new Error(`${document.documentElement.dataset.theme}: Agent draws a border while aria-pressed=${pressed}`);
      }
    }
    document.body.classList.add('project-board-open');
    try {
      const workspaceBack = document.getElementById('back-to-workspace');
      const bounds = workspaceBack.getBoundingClientRect();
      if (!closeEnough(bounds.left, 0) || getComputedStyle(workspaceBack).backgroundColor !== 'rgba(0, 0, 0, 0)') {
        throw new Error(`${document.documentElement.dataset.theme}: Projects back arrow is not flush left on a transparent surface`);
      }
    } finally {
      document.body.classList.remove('project-board-open');
    }
  };
  const verifyStartPage = () => {
    const mark = document.querySelector('.mark-slot').getBoundingClientRect();
    const wordmark = document.querySelector('h1').getBoundingClientRect();
    const tagline = document.querySelector('.tagline').getBoundingClientRect();
    const firstGap = wordmark.top - mark.bottom;
    const secondGap = tagline.top - wordmark.bottom;
    if (!closeEnough(firstGap, secondGap) || firstGap <= 0) {
      throw new Error(`Home brand spacing is asymmetric: ${JSON.stringify({firstGap,secondGap})}`);
    }
    const form = document.querySelector('form');
    form.querySelector('input').focus();
    const focused = getComputedStyle(form);
    if (focused.borderTopColor !== 'rgba(0, 0, 0, 0)' || focused.boxShadow === 'none') {
      throw new Error('Home search must show only its outer focus ring');
    }
    if (getComputedStyle(form.querySelector('input')).outlineStyle !== 'none') {
      throw new Error('Home search input draws an inner focus outline');
    }
  };
  for (const theme of ['central', 'central_dark']) {
    document.documentElement.dataset.theme = theme;
    if (startupSurface === 'toolbar') verifyToolbar();
    if (startupSurface === 'start-page') verifyStartPage();
  }
} catch (error) { startupErrors.push(String(error)); }
