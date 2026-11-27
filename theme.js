/* Lotus theme picker: 4 labeled buttons, persisted under lotus-theme-dm,
   default follows prefers-color-scheme. Each page's head snippet applies
   the saved theme before first paint; this file wires the buttons. */
(function () {
  var KEY = "lotus-theme-dm";
  var THEMES = [
    { id: "dark", label: "Dark" },
    { id: "light", label: "Light" },
    { id: "amoled", label: "Amoled" },
    { id: "hc", label: "High contrast" }
  ];

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

  function apply(theme, persist) {
    document.documentElement.setAttribute("data-theme", theme);
    if (persist) {
      try {
        localStorage.setItem(KEY, theme);
      } catch (e) {}
    }
    var btns = document.querySelectorAll("[data-theme-btn]");
    for (var i = 0; i < btns.length; i++) {
      var on = btns[i].getAttribute("data-theme-btn") === theme;
      btns[i].classList.toggle("on", on);
      btns[i].setAttribute("aria-pressed", on ? "true" : "false");
    }
  }

  function build() {
    var host = document.getElementById("theme-picker");
    if (!host) {
      return;
    }
    for (var i = 0; i < THEMES.length; i++) {
      (function (theme) {
        var btn = document.createElement("button");
        btn.type = "button";
        btn.className = "theme-btn";
        btn.setAttribute("data-theme-btn", theme.id);
        btn.setAttribute("aria-pressed", "false");
        btn.textContent = theme.label;
        btn.addEventListener("click", function () {
          apply(theme.id, true);
        });
        host.appendChild(btn);
      })(THEMES[i]);
    }
    apply(document.documentElement.getAttribute("data-theme") || "dark", false);
  }

  /* Follow the OS theme only while the user has not chosen one. */
  if (window.matchMedia) {
    var mq = window.matchMedia("(prefers-color-scheme: light)");
    var onChange = function () {
      if (!saved()) {
        apply(systemTheme(), false);
      }
    };
    if (mq.addEventListener) {
      mq.addEventListener("change", onChange);
    } else if (mq.addListener) {
      mq.addListener(onChange);
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", build);
  } else {
    build();
  }
})();
