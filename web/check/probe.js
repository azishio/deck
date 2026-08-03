// Layout probe evaluated inside a slide page by `deck check`.
// Returns a JSON-serialisable report; never throws.
(function runDeckProbe(config) {
  const diagnostics = [];
  const canvas = config.canvas;
  const tolerance = config.overflowTolerancePx;
  const severityOf = (rule) => config.rules[rule] ?? "error";

  const GENERIC_FAMILIES = new Set([
    "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui",
    "ui-serif", "ui-sans-serif", "ui-monospace", "ui-rounded", "math", "emoji", "fangsong",
  ]);

  function cssPath(element) {
    if (!element || element === document.documentElement) {
      return "html";
    }
    if (element.id) {
      return "#" + CSS.escape(element.id);
    }
    const parent = element.parentElement;
    if (!parent) {
      return element.localName;
    }
    const siblings = Array.from(parent.children).filter((c) => c.localName === element.localName);
    const nth = siblings.length > 1 ? ":nth-of-type(" + (siblings.indexOf(element) + 1) + ")" : "";
    return cssPath(parent) + " > " + element.localName + nth;
  }

  function rectOf(element) {
    const rect = element.getBoundingClientRect();
    return {
      x: Math.round(rect.left * 100) / 100,
      y: Math.round(rect.top * 100) / 100,
      width: Math.round(rect.width * 100) / 100,
      height: Math.round(rect.height * 100) / 100,
    };
  }

  function isIgnored(element, rule) {
    for (let node = element; node && node !== document.documentElement; node = node.parentElement) {
      if (node.hasAttribute && node.hasAttribute("data-deck-check-ignore")) {
        const value = node.getAttribute("data-deck-check-ignore").trim();
        if (value === "" || value === "*") {
          return true;
        }
        if (value.split(/[\s,]+/).includes(rule)) {
          return true;
        }
      }
    }
    for (const selector of config.ignoreSelectors) {
      try {
        if (element.closest(selector)) {
          return true;
        }
      } catch {
        /* invalid user selector */
      }
    }
    return false;
  }

  function report(rule, element, message, extra) {
    const severity = severityOf(rule);
    if (severity === "off") {
      return;
    }
    if (element && isIgnored(element, rule)) {
      return;
    }
    diagnostics.push(
      Object.assign(
        {
          rule,
          severity,
          message,
          selector: element ? cssPath(element) : null,
          rect: element ? rectOf(element) : null,
        },
        extra || {},
      ),
    );
  }

  function isVisible(element, style) {
    if (style.display === "none" || style.visibility === "hidden") {
      return false;
    }
    if (Number.parseFloat(style.opacity) === 0) {
      return false;
    }
    const rect = element.getBoundingClientRect();
    return rect.width > 0.5 && rect.height > 0.5;
  }

  function clipsAxis(overflow) {
    return overflow === "hidden" || overflow === "clip" || overflow === "auto" || overflow === "scroll";
  }

  function clipsOwnContent(style) {
    return clipsAxis(style.overflowX) || clipsAxis(style.overflowY);
  }

  function hasOwnText(element) {
    for (const node of element.childNodes) {
      if (node.nodeType === Node.TEXT_NODE && node.textContent.trim() !== "") {
        return true;
      }
    }
    return false;
  }

  function relativeLuminance(rgb) {
    const channel = (value) => {
      const c = value / 255;
      return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
    };
    return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
  }

  function parseColor(value) {
    const match = /rgba?\(([^)]+)\)/.exec(value || "");
    if (!match) {
      return null;
    }
    const parts = match[1].split(/[\s,/]+/).filter(Boolean).map(Number);
    if (parts.length < 3 || parts.some((n) => !Number.isFinite(n))) {
      return null;
    }
    return { rgb: [parts[0], parts[1], parts[2]], alpha: parts.length > 3 ? parts[3] : 1 };
  }

  function effectiveBackground(element) {
    for (let node = element; node; node = node.parentElement) {
      const color = parseColor(getComputedStyle(node).backgroundColor);
      if (color && color.alpha > 0.9) {
        return color.rgb;
      }
    }
    return [255, 255, 255];
  }

  function contrastRatio(a, b) {
    const la = relativeLuminance(a);
    const lb = relativeLuminance(b);
    const light = Math.max(la, lb);
    const dark = Math.min(la, lb);
    return (light + 0.05) / (dark + 0.05);
  }

  /* ---------------------------------------------------------------- */

  // The slide root fills the canvas by definition, so it is never a candidate
  // for the outside-canvas / safe-area rules; only its descendants are.
  const root = document.querySelector("deck-slide") || document.body;
  const elements = Array.from(root.querySelectorAll("*"));

  // slide-overflow ---------------------------------------------------
  const overflowX = root.scrollWidth - canvas.width;
  const overflowY = root.scrollHeight - canvas.height;
  if (overflowX > tolerance || overflowY > tolerance) {
    report(
      "slide_overflow",
      root,
      "slide overflows the " + canvas.width + "x" + canvas.height + " canvas by +" +
        Math.max(overflowX, 0) + "px / +" + Math.max(overflowY, 0) + "px",
    );
  }

  const textElements = [];
  let totalCharacters = 0;
  const seenFamilies = new Map();

  for (const element of elements) {
    let style;
    try {
      style = getComputedStyle(element);
    } catch {
      continue;
    }
    if (!isVisible(element, style)) {
      continue;
    }
    const rect = element.getBoundingClientRect();

    // outside-canvas / outside-safe-area ------------------------------
    const outLeft = -rect.left;
    const outTop = -rect.top;
    const outRight = rect.right - canvas.width;
    const outBottom = rect.bottom - canvas.height;
    const worstOutside = Math.max(outLeft, outTop, outRight, outBottom);
    if (worstOutside > tolerance) {
      report("outside_canvas", element, "element extends past the canvas by " + Math.round(worstOutside) + "px");
    } else {
      const safe = config.safeArea;
      const safeOut = Math.max(
        safe.left - rect.left,
        safe.top - rect.top,
        rect.right - (canvas.width - safe.right),
        rect.bottom - (canvas.height - safe.bottom),
      );
      if (safeOut > tolerance) {
        report("outside_safe_area", element, "element sits outside the safe area by " + Math.round(safeOut) + "px");
      }
    }

    if (!hasOwnText(element)) {
      continue;
    }

    const text = element.textContent.trim();
    totalCharacters += text.length;
    textElements.push({ element, rect, style });

    // clipped-text ----------------------------------------------------
    // Only self-clipping elements are checked here: with `overflow: visible`
    // scrollHeight merely reports layout overflow, which is not a defect.
    // Text escaping the slide is covered by outside_canvas above.
    if (style.display !== "inline" && clipsOwnContent(style)) {
      const clippedX = element.scrollWidth - element.clientWidth;
      const clippedY = element.scrollHeight - element.clientHeight;
      if (clipsAxis(style.overflowX) && element.clientWidth > 0 && clippedX > tolerance) {
        report("clipped_text", element, "text is cut off horizontally: +" + clippedX + "px");
      } else if (clipsAxis(style.overflowY) && element.clientHeight > 0 && clippedY > tolerance) {
        report("clipped_text", element, "text is cut off vertically: +" + clippedY + "px");
      }
    }

    // min-font-size ---------------------------------------------------
    const fontSize = Number.parseFloat(style.fontSize);
    if (Number.isFinite(fontSize) && fontSize < config.minFontPx) {
      report("min_font_size", element, "font size is too small: " + fontSize + "px < " + config.minFontPx + "px");
    }

    // missing-font ----------------------------------------------------
    const family = (style.fontFamily.split(",")[0] || "").trim().replace(/^["']|["']$/g, "");
    if (family && !GENERIC_FAMILIES.has(family.toLowerCase()) && !seenFamilies.has(family)) {
      let available = true;
      try {
        available = document.fonts.check('16px "' + family + '"');
      } catch {
        available = true;
      }
      seenFamilies.set(family, available);
      if (!available) {
        report("missing_font", element, "font is not available: " + family, { font: family });
      }
    }

    // low-contrast ----------------------------------------------------
    const color = parseColor(style.color);
    if (color) {
      const ratio = contrastRatio(color.rgb, effectiveBackground(element));
      const isLarge = fontSize >= 24 || (fontSize >= 18.66 && Number.parseInt(style.fontWeight, 10) >= 700);
      const required = isLarge ? 3 : 4.5;
      if (ratio < required) {
        report(
          "low_contrast",
          element,
          "contrast ratio is too low: " + ratio.toFixed(2) + " < " + required,
          { contrast: Math.round(ratio * 100) / 100 },
        );
      }
    }
  }

  // text-overlap -------------------------------------------------------
  for (let i = 0; i < textElements.length; i += 1) {
    for (let j = i + 1; j < textElements.length; j += 1) {
      const a = textElements[i];
      const b = textElements[j];
      if (a.element.contains(b.element) || b.element.contains(a.element)) {
        continue;
      }
      const overlapWidth = Math.min(a.rect.right, b.rect.right) - Math.max(a.rect.left, b.rect.left);
      const overlapHeight = Math.min(a.rect.bottom, b.rect.bottom) - Math.max(a.rect.top, b.rect.top);
      if (overlapWidth <= 1 || overlapHeight <= 1) {
        continue;
      }
      const overlapArea = overlapWidth * overlapHeight;
      const smaller = Math.min(a.rect.width * a.rect.height, b.rect.width * b.rect.height);
      if (smaller > 0 && overlapArea / smaller > 0.25) {
        report("text_overlap", a.element, "text overlaps " + cssPath(b.element), {
          other: cssPath(b.element),
        });
      }
    }
  }

  // text-density -------------------------------------------------------
  if (totalCharacters > config.maxCharacters) {
    report("text_density", root, "too much text on one slide: " + totalCharacters + " > " + config.maxCharacters);
  }

  const deck = window.deck || {};
  return {
    slideId: deck.slideId || null,
    title: document.title,
    step: deck.step ?? null,
    stepCount: deck.stepCount ?? null,
    declaredStepCount: deck.declaredStepCount ?? null,
    ready: Boolean(deck.ready),
    runtimeDiagnostics: typeof deck.diagnostics === "object" ? deck.diagnostics : [],
    diagnostics,
  };
})(__DECK_CHECK_CONFIG__);
