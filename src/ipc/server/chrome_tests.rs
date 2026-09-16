use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::niri::NewClient;
use crate::tests::Fixture;

struct Browser {
    child: Child,
    root: std::path::PathBuf,
}

impl Drop for Browser {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
#[ignore = "requires the declared isolated Chrome integration check"]
fn a_fresh_chrome_profile_honors_its_wayland_class() {
    let binary = std::env::var_os("NIRI_CHROME_TEST_BINARY").expect("declared Chrome binary");
    let root = std::env::temp_dir().join(format!(
        "niri-chrome-class-{}-{}",
        std::process::id(),
        fastrand::u64(..)
    ));
    fs::create_dir(&root).unwrap();
    let mut fixture = Fixture::new();
    fixture.add_output(1, (1280, 720));
    let (server, client) = UnixStream::pair().unwrap();
    fixture.niri().insert_client(NewClient {
        client: server,
        restricted: false,
        credentials_unknown: false,
    });
    let fd = client.as_raw_fd();
    let class = "mesh.fixture.niri.chrome.probe";
    let mut command = Command::new(binary);
    command
        .args([
            "--ozone-platform=wayland",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-sync",
            "--disable-default-apps",
            "--no-first-run",
            "--no-default-browser-check",
            "--password-store=basic",
            "--window-size=640,480",
        ])
        .arg(format!(
            "--user-data-dir={}",
            root.join("profile").display()
        ))
        .arg(format!("--class={class}"))
        .arg("about:blank")
        .env("WAYLAND_SOCKET", fd.to_string())
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(fs::File::create(root.join("stderr")).unwrap()))
        .process_group(0);
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut browser = Browser {
        child: command.spawn().unwrap(),
        root,
    };
    drop(client);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        fixture
            .event_loop
            .dispatch(Duration::from_millis(100), &mut fixture.state)
            .unwrap();
        fixture.niri_state().ipc_refresh_layout();
        let ids: Vec<_> = fixture
            .niri()
            .ipc_server
            .as_ref()
            .unwrap()
            .event_stream_state
            .borrow()
            .windows
            .windows
            .values()
            .map(|window| window.app_id.clone())
            .collect();
        if ids.iter().any(|id| id.as_deref() == Some(class)) {
            break;
        }
        if browser.child.try_wait().unwrap().is_some() || Instant::now() >= deadline {
            panic!(
                "Chrome class was not observed; app ids: {ids:?}; stderr: {}",
                fs::read_to_string(browser.root.join("stderr")).unwrap_or_default()
            );
        }
    }
}
