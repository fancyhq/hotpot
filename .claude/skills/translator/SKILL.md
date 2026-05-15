---
name: translator
description: 当用户要求翻译整个项目时使用
---

# translator

翻译一般目标为 `English` 和 `中文`，需要遵循下面的一些要求：

- 所有代码中的输出，必须翻译成英文，比如：

```rust
// 翻译前
println!("你好世界！");
bail!("错误信息");
// 翻译后
println!("Hello, world!");
bail!("Error message");
```

- 所有注释（包含行注释和所有 `doc` 注释），需要保留双语模式，如果只有英文，需要添加中文注释，如果只有中文，需要添加英文注释，不符合此规范的也需要进行优化，比如：

```rust
//! <English description of the module.>
//!
//! <中文模块描述>

/// <English short description of the function.>
/// <中文简短描述>
///
/// <English extended description, e.g. parameter descriptions, examples, etc.>
/// <中文扩展描述，如参数说明、示例等>
fn some_function(arg1: String, arg2: i32) -> Result<String> {
    // <English description>
    // <中文描述>
    todo!()
}
```
