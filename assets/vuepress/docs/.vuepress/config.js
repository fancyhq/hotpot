import { defaultTheme } from "@vuepress/theme-default";
import { defineUserConfig } from "vuepress";
import { viteBundler } from "@vuepress/bundler-vite";
import markdownItTaskLists from "markdown-it-task-lists";
import { generateSidebar, scanWorkspaces } from "./sidebar";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const docsDir = path.resolve(__dirname, "..");

// Scan once at config-load time and inject as a compile-time constant.
// The client-side <TaskIndex /> component reads __HOTPOT_TASK_INDEX__
// instead of doing any fs work in the browser.
//
// 配置加载时一次性扫描，作为编译期常量注入。
// 客户端 <TaskIndex /> 组件读取 __HOTPOT_TASK_INDEX__，无需在浏览器跑 fs。
const taskIndex = scanWorkspaces(docsDir);

export default defineUserConfig({
  lang: "en-US",

  title: "HOTPOT",
  description: "Hotpot Task Manager",

  // Use forward slashes so the absolute path survives string interpolation
  // into VuePress's generated clientConfigs.js on Windows (otherwise '\R',
  // '\h', etc. are eaten as JS escape sequences).
  // 用正斜杠避免 Windows 上反斜杠被 VuePress 生成的 clientConfigs.js
  // 当作 JS 字符串转义符吃掉。
  clientConfigFile: path.resolve(__dirname, "./client.js").replace(/\\/g, "/"),

  extendsMarkdown: (md) => {
    md.use(markdownItTaskLists, { enabled: true, label: true });
  },

  theme: defaultTheme({
    logo: "https://vuejs.press/images/hero.png",

    sidebar: generateSidebar(docsDir),

    // Show only file-level entries in the sidebar — suppress the
    // automatic expansion of the current page's h2/h3 headings.
    // 只在侧边栏展示文件级条目，关闭当前页 h2/h3 标题的自动展开。
    sidebarDepth: 0,
  }),

  // The user-level `define` field in defineUserConfig is an internal
  // plugin hook type, not a Vite define passthrough. Inject globals via
  // the bundler's viteOptions.define so Vite replaces them at build time.
  // VuePress 2 顶层 `define` 是内部插件钩子类型，不会透传到 Vite。要
  // 注入编译期常量必须走 viteBundler 的 viteOptions.define。
  bundler: viteBundler({
    viteOptions: {
      define: {
        __HOTPOT_TASK_INDEX__: JSON.stringify(taskIndex),
      },
    },
  }),
});
