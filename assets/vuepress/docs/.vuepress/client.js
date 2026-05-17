import { defineClientConfig } from "vuepress/client";
import TaskIndex from "./components/TaskIndex.vue";

/**
 * VuePress client config: register Hotpot components globally so they
 * can be referenced from any Markdown page (e.g. <TaskIndex /> in README).
 *
 * VuePress 客户端配置：把 Hotpot 自定义组件全局注册，使其可在任意
 * Markdown 页面里通过标签直接使用（如 README 里的 <TaskIndex />）。
 */
export default defineClientConfig({
  enhance({ app }) {
    app.component("TaskIndex", TaskIndex);
  },
});
