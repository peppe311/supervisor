import supervisorMark from "../../../assets/supervisor-mark.svg?raw";

// Only this bundled, repository-owned SVG can enter the legacy DOM bridge.
// Never pass provider content or user input to this factory.
export { supervisorMark };
export function createSupervisorMark(): SVGElement {
  const template = document.createElement("template");
  template.innerHTML = supervisorMark;
  return template.content.firstElementChild!.cloneNode(true) as SVGElement;
}
