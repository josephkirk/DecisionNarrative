import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./studio/router.ts";
import "./styles.css";
import { installBuiltinPlugins } from "./studio/plugins/index.ts";

installBuiltinPlugins();

createApp(App).use(router).mount("#app");
