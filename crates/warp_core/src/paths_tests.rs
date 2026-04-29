use dirs::home_dir;

use super::*;

#[test]
fn test_data_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(data_dir(), home_dir.join(".swarf-oss"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(data_dir(), home_dir.join(".local/share/swarf-oss"));
        } else if #[cfg(windows)] {
            assert_eq!(data_dir(), home_dir.join("AppData\\Roaming\\swarf\\SwarfOss\\data"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_config_local_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(config_local_dir(), home_dir.join(".swarf-oss"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(config_local_dir(), home_dir.join(".config/swarf-oss"));
        } else if #[cfg(windows)] {
            assert_eq!(config_local_dir(), home_dir.join("AppData\\Local\\swarf\\SwarfOss\\config"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_warp_home_config_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    let expected_dir_name = match ChannelState::data_profile() {
        Some(data_profile) => format!(".swarf-oss-{data_profile}"),
        None => ".swarf-oss".to_string(),
    };

    assert_eq!(
        warp_home_config_dir(),
        Some(home_dir.join(expected_dir_name))
    );
}

#[test]
fn test_warp_home_skills_and_mcp_paths() {
    let Some(config_dir) = warp_home_config_dir() else {
        panic!("Should be able to compute Swarf home config directory");
    };

    assert_eq!(warp_home_skills_dir(), Some(config_dir.join("skills")));
    assert_eq!(
        warp_home_mcp_config_file_path(),
        Some(config_dir.join(".mcp.json"))
    );
}
#[test]
fn test_cache_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    // ChannelState, by default, is configured for Channel::Oss.
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(cache_dir(), home_dir.join("Library/Application Support/dev.swarf.SwarfOss"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(cache_dir(), home_dir.join(".cache/swarf-oss"));
        } else if #[cfg(windows)] {
            assert_eq!(cache_dir(), home_dir.join("AppData\\Local\\swarf\\SwarfOss\\cache"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_state_dir_path() {
    let home_dir = home_dir().expect("Should be able to compute home directory");
    cfg_if::cfg_if! {
        // ChannelState, by default, is configured for Channel::Oss.
        if #[cfg(target_os = "macos")] {
            assert_eq!(state_dir(), home_dir.join("Library/Application Support/dev.swarf.SwarfOss"));
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(state_dir(), home_dir.join(".local/state/swarf-oss"));
        } else if #[cfg(windows)] {
            assert_eq!(state_dir(), home_dir.join("AppData\\Local\\swarf\\SwarfOss\\data"));
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_swarf_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "swarf", "Swarf"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.swarf.Swarf");
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(project_dirs.project_path(), "swarf-terminal");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "swarf\\Swarf");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_swarf_dev_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "swarf", "SwarfDev"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.swarf.SwarfDev");
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(project_dirs.project_path(), "swarf-terminal-dev");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "swarf\\SwarfDev");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}

#[test]
fn test_project_path_for_oss_app_id() {
    let project_dirs = project_dirs_for_app_id(AppId::new("dev", "swarf", "SwarfOss"), None)
        .expect("should be able to compute project dirs");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            assert_eq!(project_dirs.project_path(), "dev.swarf.SwarfOss");
        } else if #[cfg(target_os = "linux")] {
            assert_eq!(project_dirs.project_path(), "swarf-oss");
        } else if #[cfg(windows)] {
            assert_eq!(project_dirs.project_path(), "swarf\\SwarfOss");
        } else {
            unimplemented!("Need to update tests for current platform!");
        }
    }
}
