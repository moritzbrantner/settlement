export type HotkeyBinding = {
  key: string;
  run: () => void;
  enabled?: boolean | (() => boolean);
};

export function installHotkeys(getBindings: () => HotkeyBinding[]) {
  document.addEventListener("keydown", (event) => {
    if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) {
      return;
    }

    if (isEditableTarget(event.target)) {
      return;
    }

    const key = normalizeHotkey(event.key);
    if (!key) {
      return;
    }

    const binding = getBindings().find(
      (candidate) =>
        normalizeHotkey(candidate.key) === key && bindingEnabled(candidate.enabled),
    );
    if (!binding) {
      return;
    }

    event.preventDefault();
    binding.run();
  });
}

export function setButtonLabelWithHotkey(button: HTMLElement, label: string, key: string | null) {
  const labelEl = document.createElement("span");
  labelEl.textContent = label;

  if (!key) {
    button.replaceChildren(labelEl);
    button.removeAttribute("aria-keyshortcuts");
    return;
  }

  const badge = document.createElement("span");
  badge.className = "hotkey-badge";
  badge.setAttribute("aria-hidden", "true");
  badge.textContent = hotkeyLabel(key);

  button.replaceChildren(labelEl, badge);
  button.setAttribute("aria-keyshortcuts", ariaShortcut(key));
}

function hotkeyLabel(key: string) {
  const normalized = normalizeHotkey(key);
  if (normalized === "space") return "Space";
  if (!normalized) return "";
  return normalized.length === 1 ? normalized.toUpperCase() : normalized;
}

function ariaShortcut(key: string) {
  const normalized = normalizeHotkey(key);
  if (normalized === "space") return "Space";
  if (!normalized) return "";
  return normalized.length === 1 ? normalized.toUpperCase() : normalized;
}

function normalizeHotkey(key: string | null | undefined) {
  if (!key) {
    return null;
  }

  if (key === " ") {
    return "space";
  }

  return key.toLowerCase();
}

function bindingEnabled(enabled: HotkeyBinding["enabled"]) {
  if (typeof enabled === "function") {
    return enabled();
  }

  return enabled ?? true;
}

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return Boolean(target.closest("input, textarea, select, [contenteditable='true']"));
}
