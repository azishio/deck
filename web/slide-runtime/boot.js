// Entry module referenced by every slide document:
//
//   <script type="module" src="/@deck/boot.js"></script>
//
// Relative imports keep the module graph valid under any base URL, which the
// static build (`deck build --base-url`) relies on.
import { boot } from "./runtime.js";
import "./components.js";

boot();
