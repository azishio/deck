// Entry module referenced by every slide document:
//
//   <script type="module" src="/@deck/boot.js"></script>
//
// Relative imports keep the module graph valid under any base URL, which the
// static build (`deck build --base-url`) relies on.
import { boot } from "./runtime.js";

// `window.deck` has to exist before any element upgrades, because upgrading is
// what runs connectedCallback. A static import would be hoisted above this
// call, so the component module is pulled in dynamically.
boot();
await import("./components.js");
