# Hotpot Review Memory

这里记录的是历史 review 或实现中出现过的问题。执行代码 review 时，必须先根据每条记录的 `Scene` 判断当前变更是否相关。

如果当前变更匹配某条 `Scene`，必须执行对应的 `Review Check`。如果发现同类问题，参考 `Solution` 给出修复建议，避免重复引入历史问题。

示例：

```markdown
## 创建卡片显示优化

- kind: optimization
- date: 2026-05-11
- tags: ui, card, border-radius
- paths: src/components

### Scene

当修改卡片、列表项、详情面板、弹窗内容块等 UI 容器时。

### Problem

添加的卡片没有使用圆角形式，导致视觉风格不一致。

### Review Check

检查新增或修改的卡片类 UI 是否遵循项目现有圆角规范。

### Solution

给卡片添加 `10px` 的圆角边框。
```

## 测试title
- kind: bug
- date: 2026-05-11
- tags: rust, jsonl, serde
- paths: src/issues.rs

### Scene
测试场景

### Problem
测试描述

### Review Check
测试检查项

### Solution
测试解决方案

## 测试title2
- kind: optimization
- date: 2026-05-11
- tags: markdown, rendering
- paths: src/issues.rs

### Scene
测试场景2

### Problem
测试描述

### Review Check
测试检查项

### Solution
测试解决方案

## 测试title3
- kind: bug
- date: 2026-05-12
- tags: task, state
- paths: src/task.rs

### Scene
测试场景3

### Problem
测试描述

### Review Check
测试检查项

### Solution
测试解决方案

## 测试title4
- kind: bug
- date: 2026-05-13
- tags: path, workspace
- paths: src/paths.rs

### Scene
测试场景4

### Problem
测试描述

### Review Check
测试检查项

### Solution
测试解决方案

