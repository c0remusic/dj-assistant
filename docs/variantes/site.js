// Partagé par les variantes : bouton de téléchargement visant la machine du visiteur (API GitHub
// releases/latest, repli sur la page de la release) et visionneuse des captures (<dialog>).
(function () {
  var ua = navigator.userAgent;
  var isMac = /Macintosh|Mac OS X/.test(ua);
  var isWin = /Windows/.test(ua);
  var main = document.getElementById("dl-main");
  var ver = document.getElementById("dl-ver");
  var alts = document.querySelectorAll("[data-os]");
  var patterns = { win: /_x64-setup\.exe$/, "mac-arm": /_aarch64\.dmg$/, "mac-intel": /_x64\.dmg$/ };
  var mine = isWin ? "win" : isMac ? "mac-arm" : null;
  var label = { win: "pour Windows", "mac-arm": "pour Mac (Apple Silicon)", "mac-intel": "pour Mac Intel" };
  if (main && mine) main.firstChild.textContent = "Télécharger " + label[mine] + " ";
  fetch("https://api.github.com/repos/c0remusic/sift/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
    .then(function (r) { return r.ok ? r.json() : null; })
    .then(function (rel) {
      if (!rel || !rel.assets) return;
      if (ver) ver.textContent = rel.tag_name;
      var byOs = {};
      rel.assets.forEach(function (a) {
        Object.keys(patterns).forEach(function (k) { if (patterns[k].test(a.name)) byOs[k] = a.browser_download_url; });
      });
      alts.forEach(function (a) { var u = byOs[a.dataset.os]; if (u) a.href = u; });
      if (main && mine && byOs[mine]) main.href = byOs[mine];
    })
    .catch(function () {});

  var lb = document.getElementById("lb");
  if (!lb || typeof lb.showModal !== "function") return;
  var img = lb.querySelector("img");
  document.querySelectorAll(".win[data-full]").forEach(function (b) {
    b.addEventListener("click", function () { img.src = b.dataset.full; img.alt = b.getAttribute("aria-label") || ""; lb.classList.remove("is-1x"); lb.showModal(); });
  });
  img.addEventListener("click", function (e) { e.stopPropagation(); lb.classList.toggle("is-1x"); });
  lb.querySelector(".lb-close").addEventListener("click", function () { lb.close(); });
  lb.addEventListener("click", function (e) { if (e.target === lb || e.target.classList.contains("lb-inner")) lb.close(); });
})();
