use std::env;
use std::fmt::Display;
use std::path::PathBuf;
use std::process::{Command, exit};

mod lang;
use anyhow::Result;
use i18n_embed::{DesktopLanguageRequester, Localizer};
use inquire::Select;
use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};
use lang::{LANGUAGE_LOADER, localizer};

struct SSConfig {
    file: PathBuf,
}

impl Display for SSConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.file.to_string_lossy().fmt(f)
    }
}

fn init_localizer() {
    let localizer = localizer();
    let requested_languages = DesktopLanguageRequester::requested_languages();

    if let Err(error) = localizer.select(&requested_languages) {
        eprintln!("Error while loading languages for library_fluent {}", error);
    }

    // Windows Terminal doesn't support bidirectional (BiDi) text, and renders the isolate characters incorrectly.
    // This is a temporary workaround for https://github.com/microsoft/terminal/issues/16574
    // TODO: this might break BiDi text, though we don't support any writing system depending on that.
    LANGUAGE_LOADER.set_use_isolating(false);
}

fn main() -> Result<()> {
    init_localizer();

    if !rustix::process::geteuid().is_root() {
        panic!("{}", fl!("request-root"));
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
    let confpath = PathBuf::from("config.json");
    let mut configs: Vec<SSConfig> = vec![];
    let current: Option<PathBuf> = if confpath.is_symlink() {
        Some(std::fs::read_link(&confpath)?)
    } else {
        None
    };
    if let Ok(x) = std::fs::read_dir(PathBuf::from(".")) {
        for dir in x {
            if let Ok(entry) = dir {
                let path = PathBuf::from(entry.file_name());
                if path != confpath {
                    if current.is_none() || current.clone().unwrap() != path {
                        configs.push(SSConfig { file: path });
                    }
                }
            }
        }
    }

    if let Some(cur) = &current {
        configs.insert(0, SSConfig { file: cur.clone() });
    }
    let config_sel_string = fl!("select-config");
    let help = fl!("help-msg");
    let result = Select::new(&config_sel_string, configs)
        .with_help_message(&help)
        .with_render_config(render_config);
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
    if confpath.exists() {
        std::fs::remove_file(&confpath)?;
    }

    println!("{} -> config.json", new_conf);
    std::os::unix::fs::symlink(new_conf.file, &confpath)?;
    let status = Command::new("ss-tproxy").arg("start").spawn()?.wait()?;
    if !status.success() {
        exit(status.code().unwrap_or_else(|| 1));
    }
    Ok(())
}
