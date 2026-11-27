import { h, fill, notice } from "./dom.js";
import { get, post, q } from "./api.js";
import { state } from "./store.js";

export async function pick_folder(start, title) {
  try {
    const r = await post("/api/pick-folder", { start, title });
    if (r && typeof r.path === "string") return r.path;
    return null;
  } catch {
    return page_picker(start, title, null, false);
  }
}

const PACKAGE = /\.(nup|nx)$/i;

export async function pick_package(start, title) {
  try {
    const r = await post("/api/pick-folder", { start, title, file: true });
    if (r && typeof r.path === "string") return r.path;
    return null;
  } catch {
    return page_picker(start, title, PACKAGE, false);
  }
}

const KINDS = { pack: /\.otr$/i, rom: /\.(z64|n64|v64)$/i };

export async function pick_file(start, title, kind) {
  try {
    const r = await post("/api/pick-folder", { start, title, file: true, kind });
    if (r && typeof r.path === "string") return r.path;
    return null;
  } catch {
    return page_picker(start, title, KINDS[kind], true);
  }
}

// With `want`, files matching it can be picked; with `only`, other files are left out.
function page_picker(start, title, want, only) {
  return new Promise((resolve) => {
    const opener = document.activeElement;
    const body = h("div", { class: "body" });
    const input = h("input", { type: "text", "aria-label": "Folder path", spellcheck: "false", autocomplete: "off" });
    const use = h("button", { type: "button", class: "btn primary", hidden: !!want }, "Use this folder");
    const up = h("button", { type: "button", class: "btn" }, "Up");
    let here = "";
    let parent = "";

    function close(v) {
      document.removeEventListener("keydown", key, true);
      overlay.remove();
      if (opener && opener.focus) opener.focus();
      resolve(v);
    }

    function key(e) {
      if (e.key === "Escape") {
        e.preventDefault();
        close(null);
      }
    }

    async function load(p) {
      fill(body, h("p", { class: "loading" }, "Loading"));
      try {
        const r = await get(`/api/fs?path=${q(p)}`);
        here = r.path;
        parent = r.parent || "";
        input.value = here;
        use.disabled = here === "";
        up.disabled = here === "";
        const list = h("ul", { class: "ents" });
        for (const x of r.roots) {
          list.append(h("li", null, h("button", { type: "button", onclick: () => load(x.path) }, h("span", null, "▣"), x.label)));
        }
        const ents = only ? r.entries.filter((e) => e.kind === "dir" || want.test(e.name)) : r.entries;
        for (const e of ents) {
          const dir = e.kind === "dir";
          const full = here + (here.endsWith(state.info.sep) ? "" : state.info.sep) + e.name;
          const hit = !!want && !dir && want.test(e.name);
          list.append(
            h("li", null, h("button", { type: "button", disabled: !dir && !hit, onclick: () => (hit ? close(full) : load(full)) }, h("span", { "aria-hidden": "true" }, dir ? "▸" : "·"), e.name)),
          );
        }
        fill(
          body,
          r.truncated ? notice("warn", "This folder is large, only the first entries are shown. Type the path above to go deeper.") : null,
          r.roots.length === 0 && ents.length === 0 ? h("p", { class: "muted" }, "This folder is empty.") : list,
        );
      } catch (e) {
        fill(body, notice("error", e.message));
        use.disabled = true;
      }
    }

    up.onclick = () => load(parent);
    use.onclick = () => close(here);
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") load(input.value.trim());
    });

    const overlay = h(
      "div",
      { class: "overlay", onclick: (e) => e.target === overlay && close(null) },
      h(
        "div",
        { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": title },
        h("header", null, h("h2", null, title), h("div", { class: "pathrow" }, up, input, h("button", { type: "button", class: "btn", onclick: () => load(input.value.trim()) }, "Go"))),
        body,
        h("footer", null, h("button", { type: "button", class: "btn", onclick: () => close(null) }, "Cancel"), use),
      ),
    );
    document.addEventListener("keydown", key, true);
    document.body.append(overlay);
    input.focus();
    load(start || "");
  });
}
