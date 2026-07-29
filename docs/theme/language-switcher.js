// Minimal language switcher. mdBook has no built-in one, and the translated
// books are simply sibling directories, so the link is a path swap.
(() => {
  const LANGUAGES = [
    { code: "en", label: "English", prefix: "" },
    { code: "ja", label: "日本語", prefix: "ja/" },
  ];

  const root = document.documentElement.getAttribute("lang") ?? "en";
  const current = LANGUAGES.find((language) => language.code === root) ?? LANGUAGES[0];
  const other = LANGUAGES.filter((language) => language !== current);
  if (other.length === 0) {
    return;
  }

  // path/to/book[/ja]/page.html -> the same page under another language prefix.
  const path = location.pathname;
  const marker = current.prefix ? `/${current.prefix}` : "/";
  const index = current.prefix ? path.lastIndexOf(marker) : -1;
  const base = index >= 0 ? path.slice(0, index + 1) : path.slice(0, path.lastIndexOf("/") + 1);
  const page = index >= 0 ? path.slice(index + marker.length) : path.slice(base.length);

  const right = document.querySelector(".right-buttons");
  if (!right) {
    return;
  }
  for (const language of other) {
    const link = document.createElement("a");
    link.className = "language-switch";
    link.href = `${base}${language.prefix}${page}${location.hash}`;
    link.textContent = language.label;
    link.title = `Read this page in ${language.label}`;
    link.setAttribute("lang", language.code);
    right.append(link);
  }
})();
