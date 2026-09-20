pub(crate) const INSPECT_PAGE: &str = r#"
(() => {
  try {
    const registry = new Map();
    Object.defineProperty(window, "__agentBrowserElements", {
      configurable: true,
      value: registry,
      writable: false,
    });

    const compact = (value, limit = 160) => String(value ?? "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, limit);
    const isVisible = (element) => {
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return style.display !== "none"
        && style.visibility !== "hidden"
        && Number(style.opacity || 1) !== 0
        && rect.width > 0
        && rect.height > 0;
    };
    const inferRole = (element) => element.getAttribute("role")
      || ({ A: "link", BUTTON: "button", INPUT: "textbox", TEXTAREA: "textbox", SELECT: "combobox" }[element.tagName])
      || element.tagName.toLowerCase();
    const inferName = (element) => compact(
      element.getAttribute("aria-label")
      || element.innerText
      || element.getAttribute("placeholder")
      || element.getAttribute("title")
      || element.getAttribute("name")
      || (element.type === "submit" ? element.value : "")
    );

    const selector = [
      "a[href]", "button", "input:not([type='hidden'])", "textarea", "select",
      "[role='button']", "[role='link']", "[role='textbox']", "[contenteditable='true']",
      "[tabindex]:not([tabindex='-1'])"
    ].join(",");
    const seen = new Set();
    const elements = [];

    for (const element of document.querySelectorAll(selector)) {
      if (seen.has(element) || !isVisible(element)) continue;
      seen.add(element);
      const ref = `el-${elements.length + 1}`;
      const rect = element.getBoundingClientRect();
      registry.set(ref, element);
      elements.push({
        ref,
        tag: element.tagName.toLowerCase(),
        role: inferRole(element),
        name: inferName(element),
        inputType: compact(element.getAttribute("type"), 40),
        disabled: Boolean(element.disabled || element.getAttribute("aria-disabled") === "true"),
        bounds: {
          x: Math.round(rect.x), y: Math.round(rect.y),
          width: Math.round(rect.width), height: Math.round(rect.height),
        },
      });
      if (elements.length >= 160) break;
    }

    return {
      ok: true,
      data: {
        title: document.title || "",
        url: location.href,
        text: compact(document.body?.innerText, 16000),
        elements,
        viewport: { width: innerWidth, height: innerHeight, scrollX, scrollY },
      },
    };
  } catch (error) {
    return { ok: false, error: String(error?.message || error) };
  }
})()
"#;

pub(crate) const CAPTURE_TAB_CONTEXT: &str = r#"
(() => {
  try {
    const compact = (value, limit = 160) => String(value ?? "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, limit);
    const isVisible = (element) => {
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return style.display !== "none"
        && style.visibility !== "hidden"
        && Number(style.opacity || 1) !== 0
        && rect.width > 0
        && rect.height > 0;
    };
    const inferRole = (element) => element.getAttribute("role")
      || ({ A: "link", BUTTON: "button", INPUT: "textbox", TEXTAREA: "textbox", SELECT: "combobox" }[element.tagName])
      || element.tagName.toLowerCase();
    const inferName = (element) => compact(
      element.getAttribute("aria-label")
      || element.innerText
      || element.getAttribute("placeholder")
      || element.getAttribute("title")
      || element.getAttribute("name")
      || (element.type === "submit" ? element.value : "")
    );
    const absoluteUrl = (value) => {
      try { return new URL(String(value || ""), location.href).href; }
      catch (_) { return ""; }
    };
    const selector = [
      "a[href]", "button", "input:not([type='hidden'])", "textarea", "select",
      "[role='button']", "[role='link']", "[role='textbox']", "[contenteditable='true']",
      "[tabindex]:not([tabindex='-1'])"
    ].join(",");
    const seen = new Set();
    const elements = [];

    for (const element of document.querySelectorAll(selector)) {
      if (seen.has(element) || !isVisible(element)) continue;
      seen.add(element);
      elements.push({
        tag: element.tagName.toLowerCase(),
        role: inferRole(element),
        name: inferName(element),
        inputType: compact(element.getAttribute("type"), 40),
        disabled: Boolean(element.disabled || element.getAttribute("aria-disabled") === "true"),
      });
      if (elements.length >= 160) break;
    }

    const links = [];
    const seenLinks = new Set();
    for (const anchor of document.querySelectorAll("a[href]")) {
      if (!isVisible(anchor)) continue;
      const href = absoluteUrl(anchor.href);
      if (!href || seenLinks.has(href)) continue;
      seenLinks.add(href);
      links.push({
        href,
        text: compact(anchor.innerText || anchor.getAttribute("aria-label") || anchor.title, 180),
      });
      if (links.length >= 240) break;
    }

    const images = [];
    const seenImages = new Set();
    const addImage = (src, alt, width, height, kind) => {
      const resolved = absoluteUrl(src);
      if (!resolved || seenImages.has(resolved)) return;
      seenImages.add(resolved);
      images.push({
        src: resolved,
        alt: compact(alt, 180),
        width: Math.max(0, Math.round(Number(width) || 0)),
        height: Math.max(0, Math.round(Number(height) || 0)),
        kind,
      });
    };
    for (const image of document.querySelectorAll("img")) {
      if (!isVisible(image)) continue;
      addImage(
        image.currentSrc || image.src,
        image.alt || image.getAttribute("aria-label") || image.title,
        image.naturalWidth || image.width,
        image.naturalHeight || image.height,
        "image"
      );
      if (images.length >= 120) break;
    }
    if (images.length < 120) {
      for (const video of document.querySelectorAll("video[poster]")) {
        if (!isVisible(video)) continue;
        addImage(video.poster, video.getAttribute("aria-label") || video.title, video.videoWidth || video.clientWidth, video.videoHeight || video.clientHeight, "poster");
        if (images.length >= 120) break;
      }
    }
    if (images.length < 120) {
      let scanned = 0;
      for (const element of document.querySelectorAll("body *")) {
        if (++scanned > 2000 || images.length >= 120) break;
        if (!isVisible(element)) continue;
        const background = getComputedStyle(element).backgroundImage;
        if (!background || background === "none") continue;
        for (const match of background.matchAll(/url\(["']?(.*?)["']?\)/g)) {
          const rect = element.getBoundingClientRect();
          addImage(match[1], element.getAttribute("aria-label") || element.title, rect.width, rect.height, "background");
          if (images.length >= 120) break;
        }
      }
    }

    const root = document.documentElement;
    const body = document.body;
    const pageWidth = Math.max(innerWidth, root?.scrollWidth || 0, root?.offsetWidth || 0, body?.scrollWidth || 0, body?.offsetWidth || 0);
    const pageHeight = Math.max(innerHeight, root?.scrollHeight || 0, root?.offsetHeight || 0, body?.scrollHeight || 0, body?.offsetHeight || 0);

    return {
      ok: true,
      data: {
        title: document.title || "",
        url: location.href,
        description: document.querySelector('meta[name="description"]')?.content || "",
        language: document.documentElement?.lang || "",
        text: compact(document.body?.innerText, 60000),
        elements,
        links,
        images,
        page: { width: pageWidth, height: pageHeight },
      },
    };
  } catch (error) {
    return { ok: false, error: String(error?.message || error) };
  }
})()
"#;

const CLICK_TEMPLATE: &str = r#"
(() => {
  try {
    const ref = __TARGET__;
    const element = window.__agentBrowserElements?.get(ref);
    if (!element || !element.isConnected) {
      return { ok: false, error: "Expired reference: inspect the page again" };
    }
    if (element.disabled || element.getAttribute("aria-disabled") === "true") {
      return { ok: false, error: "L'elemento e disabilitato" };
    }
    element.scrollIntoView({ block: "center", inline: "center" });
    element.focus({ preventScroll: true });
    element.click();
    return { ok: true, data: { target: ref, action: "click" } };
  } catch (error) {
    return { ok: false, error: String(error?.message || error) };
  }
})()
"#;

const SET_TEXT_TEMPLATE: &str = r#"
(() => {
  try {
    const ref = __TARGET__;
    const value = __VALUE__;
    const element = window.__agentBrowserElements?.get(ref);
    if (!element || !element.isConnected) {
      return { ok: false, error: "Expired reference: inspect the page again" };
    }
    element.scrollIntoView({ block: "center", inline: "center" });
    element.focus({ preventScroll: true });

    if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
      const prototype = element instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
      setter ? setter.call(element, value) : (element.value = value);
    } else if (element.isContentEditable) {
      element.textContent = value;
    } else {
      return { ok: false, error: "L'elemento non accetta testo" };
    }

    element.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: value }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    return { ok: true, data: { target: ref, action: "set_text", length: value.length } };
  } catch (error) {
    return { ok: false, error: String(error?.message || error) };
  }
})()
"#;

const SCROLL_TEMPLATE: &str = r#"
(() => {
  try {
    const deltaY = __DELTA__;
    window.scrollBy({ top: deltaY, left: 0, behavior: "smooth" });
    return { ok: true, data: { deltaY, scrollY: window.scrollY } };
  } catch (error) {
    return { ok: false, error: String(error?.message || error) };
  }
})()
"#;

pub(crate) fn click(target: &str) -> String {
    CLICK_TEMPLATE.replace("__TARGET__", &json_string(target))
}

pub(crate) fn set_text(target: &str, value: &str) -> String {
    SET_TEXT_TEMPLATE
        .replace("__TARGET__", &json_string(target))
        .replace("__VALUE__", &json_string(value))
}

pub(crate) fn scroll(delta_y: i32) -> String {
    SCROLL_TEMPLATE.replace("__DELTA__", &delta_y.clamp(-5_000, 5_000).to_string())
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("una stringa Rust e sempre serializzabile in JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_arguments_are_json_escaped() {
        let script = set_text("el-'\"", "riga 1\n</script>");
        assert!(script.contains(r#""el-'\"""#));
        assert!(script.contains(r#""riga 1\n</script>""#));
        assert!(!script.contains("const value = riga 1"));
    }

    #[test]
    fn scroll_is_bounded() {
        let script = scroll(i32::MAX);
        assert!(script.contains("const deltaY = 5000"));
    }
}
