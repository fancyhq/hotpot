<!--
  Task index rendered on the home page below the hero block.
  Reads __HOTPOT_TASK_INDEX__ — a compile-time constant injected by
  config.js via viteBundler.viteOptions.define. No fs access in the browser.

  首页 hero 区块下方渲染的任务索引组件。
  数据来源是编译期常量 __HOTPOT_TASK_INDEX__，由 config.js 通过
  viteBundler.viteOptions.define 注入，浏览器侧不做任何文件系统访问。
-->
<script setup>
import { computed } from "vue";

// Fall back to [] if the constant is somehow missing (e.g. accidental dev run
// without the inject step) so the page still renders.
// 兜底为空数组，防止 inject 步骤被遗漏时整页崩溃。
const workspaces =
  typeof __HOTPOT_TASK_INDEX__ === "undefined" ? [] : __HOTPOT_TASK_INDEX__;

const totalTasks = computed(() =>
  workspaces.reduce((sum, ws) => sum + ws.tasks.length, 0),
);
</script>

<template>
  <section class="hotpot-task-index">
    <h2 class="hotpot-title">
      <span>Tasks</span>
      <span v-if="workspaces.length > 0" class="hotpot-total">
        {{ totalTasks }}
      </span>
    </h2>

    <p v-if="workspaces.length === 0" class="hotpot-empty">
      No tasks yet. Run <code>hotpot task create</code> to add one.
    </p>

    <div v-else class="hotpot-grid">
      <section
        v-for="ws in workspaces"
        :key="ws.user"
        class="hotpot-user-card"
      >
        <header class="hotpot-user-header">
          <!-- User icon (SVG, no external dep) | 用户图标，纯 SVG 无外部依赖 -->
          <svg
            class="hotpot-user-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
            <circle cx="12" cy="7" r="4" />
          </svg>
          <span class="hotpot-user-name">{{ ws.user }}</span>
          <span class="hotpot-task-count">{{ ws.tasks.length }}</span>
        </header>

        <ul class="hotpot-task-list">
          <li
            v-for="task in ws.tasks"
            :key="task.link"
            class="hotpot-task-item"
          >
            <RouterLink class="hotpot-task-link" :to="task.link">
              <!-- Chevron right icon | 右向箭头图标 -->
              <svg
                class="hotpot-task-chevron"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <polyline points="9 18 15 12 9 6" />
              </svg>
              <span class="hotpot-task-name">{{ task.name }}</span>
            </RouterLink>
          </li>
        </ul>
      </section>
    </div>
  </section>
</template>

<style scoped>
.hotpot-task-index {
  max-width: 1100px;
  margin: 0 auto;
  padding: 0 1.5rem 2rem;
}

/* Reset default-theme h2 top spacing — the home layout already adds breathing
   room above the markdown body, we don't need another 2rem on top of that. */
/* 重置默认主题 h2 顶部间距，home 布局上方已有空间，无需再叠 2rem。 */
.hotpot-title {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  margin: 0.5rem 0 1.4rem;
  padding-bottom: 0.55rem;
  border-bottom: 1px solid var(--vp-c-divider);
  font-size: 1.45rem;
  font-weight: 600;
}

.hotpot-title::before {
  content: "";
  display: inline-block;
  width: 4px;
  height: 1.1rem;
  border-radius: 2px;
  background: var(--vp-c-accent);
}

.hotpot-total {
  font-size: 0.82rem;
  font-weight: 500;
  color: var(--vp-c-text-mute);
  background: var(--vp-c-bg-alt);
  border: 1px solid var(--vp-c-divider);
  padding: 0.1rem 0.55rem;
  border-radius: 999px;
}

.hotpot-empty {
  color: var(--vp-c-text-mute);
  font-style: italic;
}

.hotpot-empty code {
  font-style: normal;
  padding: 0.1rem 0.4rem;
  background: var(--vp-c-bg-alt);
  border-radius: 4px;
  font-size: 0.9em;
}

.hotpot-grid {
  display: grid;
  gap: 1.2rem;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
}

.hotpot-user-card {
  display: flex;
  flex-direction: column;
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
  overflow: hidden;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.hotpot-user-card:hover {
  border-color: var(--vp-c-accent);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.06);
}

/* Header: distinct background strip so the user name is clearly the card title
   and not just another link in the list. */
/* Header 用独立背景条，让用户名作为卡片标题，与下方任务名清晰区分。 */
.hotpot-user-header {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  padding: 0.7rem 0.95rem;
  background: var(--vp-c-bg-alt);
  border-bottom: 1px solid var(--vp-c-divider);
}

.hotpot-user-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  color: var(--vp-c-accent);
}

.hotpot-user-name {
  flex: 1;
  font-weight: 600;
  font-size: 1rem;
  color: var(--vp-c-text);
  word-break: break-word;
}

.hotpot-task-count {
  font-size: 0.78rem;
  font-weight: 500;
  color: var(--vp-c-text-mute);
  background: var(--vp-c-bg);
  border: 1px solid var(--vp-c-divider);
  padding: 0.05rem 0.5rem;
  border-radius: 999px;
}

.hotpot-task-list {
  list-style: none;
  margin: 0;
  padding: 0.4rem 0;
}

.hotpot-task-item {
  margin: 0;
}

/* Make each task look like a real button row: full-width hit area, chevron
   icon as affordance, accent color + slide animation on hover. */
/* 让任务项呈现为按钮行：整行命中、chevron 图标提示可点击、hover 时
   高亮 + 轻微位移。 */
.hotpot-task-link {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.95rem;
  color: var(--vp-c-text);
  text-decoration: none;
  font-size: 0.92rem;
  line-height: 1.4;
  transition: background-color 0.15s, color 0.15s;
}

.hotpot-task-link:hover {
  background: var(--vp-c-bg-alt);
  color: var(--vp-c-accent);
  text-decoration: none;
}

.hotpot-task-chevron {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  opacity: 0.45;
  transition: opacity 0.15s, transform 0.15s, color 0.15s;
}

.hotpot-task-link:hover .hotpot-task-chevron {
  opacity: 1;
  transform: translateX(2px);
  color: var(--vp-c-accent);
}

.hotpot-task-name {
  word-break: break-word;
}
</style>
