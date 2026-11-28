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

  function parse(t) {
    if (t === "auto") return { base: "system", amoled: false, hc: false };
    if (t === "hc") return { base: "dark", amoled: false, hc: true };
    if (t === "hc-light") return { base: "light", amoled: false, hc: true };
    if (t === "amoled") return { base: "dark", amoled: true, hc: false };
    if (t === "light") return { base: "light", amoled: false, hc: false };
    return { base: "dark", amoled: false, hc: false };
  }

  function baseOf(s) {
    return s.base === "system" ? systemTheme() : s.base;
  }

  function resolve(s) {
    var base = baseOf(s);
    if (s.hc) return base === "light" ? "hc-light" : "hc";
    if (base === "dark") return s.amoled ? "amoled" : "dark";
    return "light";
  }

  function intent(s) {
    if (s.base === "system" && !s.hc && !s.amoled) return "auto";
    return resolve(s);
  }

  function label(s) {
    if (s.hc) return "High contrast";
    if (s.base === "system") return "System";
    if (s.base === "light") return "Light";
    return s.amoled ? "Amoled" : "Dark";
  }

  var state = parse(document.documentElement.getAttribute("data-theme") || "dark");
  var pinned = saved();
  if (!pinned || pinned === "auto") state.base = "system";
  var trigger, menu, lab, modeBtns, amoledIn, hcIn;

  function sync() {
    lab.textContent = label(state);
    for (var i = 0; i < modeBtns.length; i++) {
      var on = modeBtns[i].getAttribute("data-mode") === state.base;
      modeBtns[i].classList.toggle("on", on);
      modeBtns[i].setAttribute("aria-pressed", on ? "true" : "false");
    }
    var dark = baseOf(state) === "dark";
    amoledIn.parentNode.hidden = !dark;
    amoledIn.checked = state.amoled;
    hcIn.checked = state.hc;
  }

  function apply(persist) {
    document.documentElement.setAttribute("data-theme", resolve(state));
    if (persist) {
      try {
        localStorage.setItem(KEY, intent(state));
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
    [["system", "System"], ["dark", "Dark"], ["light", "Light"]].forEach(function (pair) {
      var m = pair[0];
      var b = el("button", "mode-btn", pair[1]);
      b.type = "button";
      b.setAttribute("data-mode", m);
      b.addEventListener("click", function () {
        state.base = m;
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
      if (state.base === "system") state.base = baseOf(state);
      if (state.base !== "dark") { amoledIn.checked = false; return; }
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

  if (window.matchMedia) {
    var mq = window.matchMedia("(prefers-color-scheme: light)");
    var onChange = function () {
      var pinned = saved();
      if (!pinned || pinned === "auto") {
        state.base = "system";
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
