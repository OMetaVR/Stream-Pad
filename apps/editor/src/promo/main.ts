// promo.html entry. The director (default) renders the 15 s promo and exposes `window.promo`; it embeds the
// real editor app in an iframe (`?part=editor`) so the app's portals, dialogs and layout are untouched.

const part = new URLSearchParams(location.search).get("part");
if (part === "editor") {
  document.documentElement.dataset.promo = "editor";
  void import("./editor/boot");
}
else void import("./director/boot");
