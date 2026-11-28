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

  var base = "https:

  primary.href = base + "LotusDM-android-universal.apk";
  primary.textContent = "Download for Android";

  menu.innerHTML =
    '<a href="' + base + 'LotusDM-android-arm64-v8a.apk">arm64-v8a .apk</a>' +
    '<a href="' + base + 'LotusDM-android-armeabi-v7a.apk">armeabi-v7a .apk</a>' +
    '<a href="' + base + 'LotusDM-android-x86_64.apk">x86_64 .apk</a>';

  if (toggle) toggle.setAttribute("aria-label", "Other Android builds");
})();
