use crate::atomic_file;
use crate::config::Config;
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;

pub(crate) fn parse_user_packages(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("package:"))
        .filter_map(|line| line.rsplit_once('='))
        .filter(|(path, package)| path.starts_with("/data/app/") && !package.is_empty())
        .map(|(_, package)| package.to_owned())
        .collect()
}

pub(crate) fn generate(config: &Config) -> Result<()> {
    logging::log(
        Level::Info,
        "正在获取已安装包列表... [Listing installed packages...]",
    );
    let output = process::output("cmd", ["package", "list", "packages", "-f"])?;
    let text = process::stdout_text(output, "获取包列表失败 [Failed to list packages]")?;
    let packages = parse_user_packages(&text);
    let mut rendered = packages.join("\n");
    if !rendered.is_empty() {
        rendered.push('\n');
    }
    atomic_file::write(&config.target_txt, rendered.as_bytes()).map_err(|error| {
        TfError::new(format!(
            "无法更新 {} [Failed to update {}]: {error}",
            config.target_txt.display(),
            config.target_txt.display()
        ))
    })?;
    logging::log(
        Level::Info,
        format!(
            "已写入 {} 个包到 {} [Wrote {} packages to {}]",
            packages.len(),
            config.target_txt.display(),
            packages.len(),
            config.target_txt.display()
        ),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_user_apps_and_has_no_fixed_limit() {
        let fixture = include_str!("../../../tests/fixtures/packages.txt");
        assert_eq!(parse_user_packages(fixture), ["com.example.user"]);

        let mut input = String::from("package:/system/app/System.apk=com.system\n");
        for index in 0..2_100 {
            input.push_str(&format!(
                "package:/data/app/~~token/app{index}/base.apk=com.example.app{index}\n"
            ));
        }
        let packages = parse_user_packages(&input);
        assert_eq!(packages.len(), 2_100);
        assert_eq!(packages[0], "com.example.app0");
        assert_eq!(packages[2_099], "com.example.app2099");
    }
}
