import { createApp } from "vue";
import App from "./App.vue";
import { nativeAssistant } from "./assistant/native";
import "@rss-mdm-agent/ui/style.css";
import "./style.css";
createApp(App, { assistantServices: nativeAssistant() }).mount("#app");
