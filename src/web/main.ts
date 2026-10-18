import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./studio/router.ts";
import "./styles.css";

createApp(App).use(router).mount("#app");
