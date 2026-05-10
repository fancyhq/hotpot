use std::env;

use anyhow::{Context, Result};

/// 从环境变量获取Agent打开的根目录,此变量由Hooks进行设置
pub fn get_root_dir() -> Result<String> {
    env::var("ROOT_DIR").context("没有找到 ROOT_DIR 环境变量")
}

/// 从环境变量中获取用户名，此变量在Hooks中设置
pub fn get_username() -> Result<String> {
    env::var("HOTPOT_USERNAME").context("没有找到 HOTPOT_USERNAME 环境变量")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_root_dir() {
        let result = get_root_dir().unwrap();
        println!("{}", result)
    }
}
