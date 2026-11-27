/* Lotus theme menu: one expandable button - Dark / Light segment,
   Amoled toggle (dark only), High contrast toggle (always). Resolved
   theme persists under lotus-theme-dm; each page's head snippet
   applies it before first paint. */
(function () {
  var KEY = "lotus-theme-dm";

  function systemTheme() {
    return window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: light)").matches
      ? "light"
      : "dark";
  }

  function saved() {
    try {
      return localStorage.getItem(KEY);
    } catch (e) {
      return null;
    }
  }

  /* resolved data-theme -> menu state */
  function parse(t) {
    if (t === "hc") return { base: "dark", amoled: false, hc: true };
    if (t === "hc-light") return { base: "light", amoled: false, hc: true };
    if (t === "amoled") return { base: "dark", amoled: true, hc: false };
    if (t === "light") return { base: "light", amoled: false, hc: false };
    return { base: "dark", amoled: false, hc: false };
  }

  /* menu state -> resolved data-theme; HC follows the base mode */
  function resolve(s) {
    if (s.hc) return s.base === "light" ? "hc-light" : "hc";
    if (s.base === "dark") return s.amoled ? "amoled" : "dark";
    return "light";
  }

  function label(s) {
    if (s.hc) return "High contrast";
    if (s.base === "light") return "Light";
    return s.amoled ? "Amoled" : "Dark";
  }

  var state = parse(document.documentElement.getAttribute("data-theme") || "dark");
  var trigger, menu, lab, modeBtns, amoledIn, hcIn;

  function sync() {
    lab.textContent = label(state);
    for (var i = 0; i < modeBtns.length; i++) {
      var on = modeBtns[i].getAttribute("data-mode") === state.base;
      modeBtns[i].classList.toggle("on", on);
      modeBtns[i].setAttribute("aria-pressed", on ? "true" : "false");
    }
    var dark = state.base === "dark";
    amoledIn.parentNode.hidden = !dark;
    amoledIn.checked = state.amoled;
    hcIn.checked = state.hc;
  }

  function apply(persist) {
    document.documentElement.setAttribute("data-theme", resolve(state));
    if (persist) {
      try {
        localStorage.setItem(KEY, resolve(state));
      } catch (e) {}
    }
    sync();
  }

  function close() {
    menu.hidden = true;
    trigger.setAttribute("aria-expanded", "false");
  }

  function toggleMenu() {
    menu.hidden = !menu.hidden;
    trigger.setAttribute("aria-expanded", menu.hidden ? "false" : "true");
  }

  function el(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text) n.textContent = text;
    return n;
  }

  function build() {
    var host = document.getElementById("theme-picker");
    if (!host) return;

    trigger = el("button", "theme-toggle");
    trigger.type = "button";
    trigger.setAttribute("aria-haspopup", "true");
    trigger.setAttribute("aria-expanded", "false");
    lab = el("span", "theme-label");
    trigger.appendChild(lab);
    trigger.appendChild(el("span", "chev", "\u25be"));

    menu = el("div", "theme-menu");
    menu.hidden = true;

    var seg = el("div", "theme-seg");
    modeBtns = [];
    ["dark", "light"].forEach(function (m) {
      var b = el("button", "mode-btn", m === "dark" ? "Dark" : "Light");
      b.type = "button";
      b.setAttribute("data-mode", m);
      b.addEventListener("click", function () {
        state.base = m;               /* HC follows the picked mode */
        if (m === "light") state.amoled = false;
        apply(true);
      });
      seg.appendChild(b);
      modeBtns.push(b);
    });
    menu.appendChild(seg);

    var amoledLbl = el("label", "theme-opt");
    amoledIn = document.createElement("input");
    amoledIn.type = "checkbox";
    amoledLbl.appendChild(amoledIn);
    amoledLbl.appendChild(el("span", "opt-txt", "Amoled"));
    amoledLbl.appendChild(el("span", "sw"));
    amoledIn.addEventListener("change", function () {
      state.amoled = amoledIn.checked;
      if (amoledIn.checked) state.hc = false;
      apply(true);
    });
    menu.appendChild(amoledLbl);

    var hcLbl = el("label", "theme-opt");
    hcIn = document.createElement("input");
    hcIn.type = "checkbox";
    hcLbl.appendChild(hcIn);
    hcLbl.appendChild(el("span", "opt-txt", "High contrast"));
    hcLbl.appendChild(el("span", "sw"));
    hcIn.addEventListener("change", function () {
      state.hc = hcIn.checked;
      apply(true);
    });
    menu.appendChild(hcLbl);

    trigger.addEventListener("click", function (e) {
      e.stopPropagation();
      toggleMenu();
    });
    menu.addEventListener("click", function (e) { e.stopPropagation(); });
    document.addEventListener("click", close);
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") close();
    });

    host.appendChild(trigger);
    host.appendChild(menu);
    apply(false);
  }

  /* Follow the OS theme only while the user has not chosen one. */
  if (window.matchMedia) {
    var mq = window.matchMedia("(prefers-color-scheme: light)");
    var onChange = function () {
      if (!saved()) {
        state = parse(systemTheme());
        apply(false);
      }
    };
    if (mq.addEventListener) mq.addEventListener("change", onChange);
    else if (mq.addListener) mq.addListener(onChange);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", build);
  } else {
    build();
  }
})();
