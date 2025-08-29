use std::env;
use std::fmt::Display;
use std::path::PathBuf;
use std::process::{Command, exit};

use anyhow::Result;
use inquire::Select;
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};

struct SSConfig {
    file: PathBuf,
}

impl Display for SSConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.file.to_string_lossy().fmt(f)
    }
}

fn main() -> Result<()> {
    if !rustix::process::geteuid().is_root() {
        panic!("Please run as root");
    }
    let render_config = RenderConfig {
        help_message: StyleSheet::empty().with_fg(Color::LightBlue),
        highlighted_option_prefix: Styled::new("👉"),
        selected_option: Some(StyleSheet::new().with_fg(Color::DarkCyan)),
        scroll_down_prefix: Styled::new("▼"),
        scroll_up_prefix: Styled::new("▲"),
        ..Default::default()
    };
    let sspath: PathBuf = PathBuf::from("/etc/shadowsocks");
    env::set_current_dir(sspath)?;
    let mut configs: Vec<SSConfig> = vec![];
    let mut current: Option<PathBuf> = None;
    if let Ok(x) = std::fs::read_dir(PathBuf::from(".")) {
        for dir in x {
            if let Ok(entry) = dir {
                let path = PathBuf::from(entry.file_name());
                if path == PathBuf::from("config.json") {
                    if path.is_symlink() {
                        current.replace(std::fs::read_link(path)?);
                    }
                } else if current.is_none() || current.clone().unwrap() != path {
                    configs.push(SSConfig { file: path });
                }
            }
        }
    }

    if let Some(cur) = &current {
        configs.insert(0, SSConfig { file: cur.clone() });
    }
    let result = Select::new("select your config", configs).with_render_config(render_config);
    let new_conf = result.prompt()?;
    println!(
        "Old config: {}",
        if let Some(c) = current {
            c.display().to_string()
        } else {
            String::from("None")
        }
    );

    let status = Command::new("ss-tproxy").arg("stop").spawn()?.wait()?;
    if !status.success() {
        exit(status.code().unwrap_or_else(|| 1));
    }
    std::fs::remove_file("config.json")?;

    println!("{} -> config.json", new_conf);
    std::os::unix::fs::symlink(new_conf.file, "config.json")?;
    let status = Command::new("ss-tproxy").arg("start").spawn()?.wait()?;
    if !status.success() {
        exit(status.code().unwrap_or_else(|| 1));
    }
    Ok(())
}
