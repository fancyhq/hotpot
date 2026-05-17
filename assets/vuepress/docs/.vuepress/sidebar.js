import fs from "fs";
import path from "path";

/**
 * Scan the docs directory for per-user task workspaces.
 * 扫描 docs 目录，收集每个用户工作区下的任务文件。
 *
 * Skips dotfiles, non-directories, and empty user folders.
 * Symlinks/junctions are resolved via fs.statSync so platform-specific
 * task mounts still appear.
 *
 * 跳过点目录、非目录条目以及空的用户目录。symlink / Windows junction
 * 通过 fs.statSync 解析，因此各平台挂载的任务目录都能枚举。
 *
 * @param {string} docsDir absolute path of the VuePress docs directory
 * @returns {Array<{ user: string, tasks: Array<{ name: string, link: string }> }>}
 *   user-grouped task entries, sorted by user name asc and task file name desc
 *   按用户名升序、任务文件名倒序（新任务在前）排列的索引数据
 */
export function scanWorkspaces(docsDir) {
  const workspaces = [];
  for (const entry of fs.readdirSync(docsDir, { withFileTypes: true })) {
    // skip .vuepress .cache .temp etc.
    // 跳过 .vuepress / .cache / .temp 等
    if (entry.name.startsWith(".")) continue;

    let isDir = entry.isDirectory();
    if (!isDir && entry.isSymbolicLink()) {
      try {
        isDir = fs.statSync(path.join(docsDir, entry.name)).isDirectory();
      } catch {
        continue;
      }
    }
    if (!isDir) continue;

    const userDir = path.join(docsDir, entry.name);
    const mds = fs
      .readdirSync(userDir)
      .filter((f) => f.endsWith(".md") && !f.startsWith("."))
      .sort((a, b) => b.localeCompare(a));
    if (mds.length === 0) continue;

    workspaces.push({
      user: entry.name,
      tasks: mds.map((f) => ({
        name: f.replace(/\.md$/, ""),
        link: `/${entry.name}/${f}`,
      })),
    });
  }
  workspaces.sort((a, b) => a.user.localeCompare(b.user));
  return workspaces;
}

/**
 * Build the VuePress default-theme sidebar from scanned workspaces.
 * 基于扫描结果构建 VuePress 默认主题侧边栏。
 *
 * @param {string} docsDir absolute path of the VuePress docs directory
 * @returns {Array<{ text: string, collapsible: boolean, children: Array<{ text: string, link: string }> }>}
 */
export function generateSidebar(docsDir) {
  return scanWorkspaces(docsDir).map(({ user, tasks }) => ({
    text: user,
    collapsible: true,
    children: tasks.map(({ name, link }) => ({ text: name, link })),
  }));
}
