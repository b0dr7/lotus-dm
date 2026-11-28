/* Split download buttons: hover opens the variant menu on desktop, the
   expand arrow opens it by click/tap (touch has no hover), and only one
   menu is open at a time. Then the platform pass: on Android the hero
   button is repointed at the universal .apk before any click. */
(function () {
  var splits = document.querySelectorAll(".split");

  function closeAll(except) {
    for (var i = 0; i < splits.length; i++) {
      if (splits[i] === except) continue;
      splits[i].classList.remove("open");
      var t = splits[i].querySelector(".split-toggle");
      if (t) t.setAttribute("aria-expanded", "false");
    }
  }

  for (var i = 0; i < splits.length; i++) {
    (function (split) {
      var toggle = split.querySelector(".split-toggle");
      if (!toggle) return;
      toggle.addEventListener("click", function (e) {
        e.stopPropagation();
        var open = split.classList.toggle("open");
        toggle.setAttribute("aria-expanded", open ? "true" : "false");
        if (open) {
          closeAll(split);
        } else {
          /* release focus so :focus-within does not hold the menu open */
          toggle.blur();
        }
      });
      split.addEventListener("focusout", function () {
        if (split.contains(document.activeElement)) return;
        split.classList.remove("open");
        toggle.setAttribute("aria-expanded", "false");
      });
    })(splits[i]);
  }

  document.addEventListener("click", function () { closeAll(null); });
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape") closeAll(null);
  });
})();

(function () {
  var ua = navigator.userAgent || "";

  var isAndroid = /Android/i.test(ua);
  if (!isAndroid) return;

  var split = document.getElementById("hero-dl");
  if (!split) return;

  var primary = split.querySelector("a.btn");
  var toggle = split.querySelector(".split-toggle");
  var menu = split.querySelector(".split-menu");
  if (!primary || !menu) return;

  var base = "https://github.com/b0dr7/lotus-dm/releases/latest/download/";

  primary.href = base + "LotusDM-android-universal.apk";
  primary.textContent = "Download for Android";

  menu.innerHTML =
    '<a href="' + base + 'LotusDM-android-arm64-v8a.apk">arm64-v8a .apk</a>' +
    '<a href="' + base + 'LotusDM-android-armeabi-v7a.apk">armeabi-v7a .apk</a>' +
    '<a href="' + base + 'LotusDM-android-x86_64.apk">x86_64 .apk</a>';

  if (toggle) toggle.setAttribute("aria-label", "Other Android builds");
})();
