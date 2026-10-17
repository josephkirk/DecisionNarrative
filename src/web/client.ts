import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./studio/router.ts";
import "./styles.css";

const app = createApp(App);
app.config.errorHandler = (err, instance, info) => {
  console.log("VUE_ERROR_STACK:", err instanceof Error ? err.stack : JSON.stringify(err), info);
};
app.use(router);
app.mount("#app");
