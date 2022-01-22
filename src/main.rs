// Don't spawn a cmd on windows
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[macro_use]
extern crate log;
extern crate simplelog;

use std::{
    fs::{self, File, OpenOptions},
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use anyhow::{bail, Result};
use directories::BaseDirs;
use ping::ping;
use simplelog::*;
use trayicon::{MenuBuilder, TrayIcon, TrayIconBuilder};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
};

const GOOGLE_DNS: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone, Eq, PartialEq, Debug)]
enum Events {
    ClickTrayIcon,
    OpenLog,
    Exit,
}

#[derive(Clone, Copy)]
enum Status {
    Up,
    Down,
}

impl<O, E> From<Result<O, E>> for Status {
    fn from(res: Result<O, E>) -> Self {
        match res {
            Ok(_) => Self::Up,
            Err(_) => Self::Down,
        }
    }
}

fn main() -> Result<()> {
    make_log_file_if_not_exists()?;

    // Get the event loop
    let event_loop = EventLoop::<Events>::with_user_event();

    // Start pinging
    spawn_worker_thread()?;

    // Let 'er rip
    start_event_loop(event_loop)?;

    Ok(())
}

fn make_tray(proxy: EventLoopProxy<Events>) -> Result<TrayIcon<Events>> {
    // At compile time pull in our icon
    let icon_buffer = include_bytes!("../resources/duckfart.ico");

    // Make a tray icon listening on the above event loop, with a few buttons mapped to Events
    let tray_icon = TrayIconBuilder::new()
        .sender_winit(proxy)
        .icon_from_buffer(icon_buffer)
        .tooltip("Fuck you Rick ;)")
        .on_click(Events::ClickTrayIcon)
        .on_double_click(Events::ClickTrayIcon)
        .menu(
            MenuBuilder::new()
                .item("Open log file", Events::OpenLog)
                .item("Exit", Events::Exit),
        )
        .build()?;

    Ok(tray_icon)
}

fn spawn_worker_thread() -> Result<()> {
    use Status::{Down, Up};

    WriteLogger::init(LevelFilter::Trace, Config::default(), get_log_file()?)?;

    info!(
        "Pinging Google DNS at {} every {} seconds",
        GOOGLE_DNS,
        INTERVAL.as_secs()
    );

    thread::spawn(move || {
        let mut last_status: Status = Up;
        let mut cur_status: Status;

        loop {
            cur_status = ping(GOOGLE_DNS, None, None, None, None, None).into();

            match (last_status, cur_status) {
                (Down, Up) => warn!("Ping started succeeding again"),
                (Up, Down) => info!("Ping started failing"),
                _ => {}
            }

            last_status = cur_status;

            thread::sleep(INTERVAL);
        }
    });

    Ok(())
}

fn start_event_loop(event_loop: EventLoop<Events>) -> Result<()> {
    let log_path = get_log_path()?;

    // Make our tray
    let tray = make_tray(event_loop.create_proxy())?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Move into here so it is cleaned up on exit
        let _ = tray;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,

            Event::UserEvent(e) => match e {
                Events::OpenLog => {
                    if let Err(e) = open::that(&log_path) {
                        eprintln!("Failed to open log file: {}", e)
                    }
                }
                Events::Exit => *control_flow = ControlFlow::Exit,
                Events::ClickTrayIcon => (),
            },
            _ => (),
        }
    });
}

fn get_log_path() -> Result<PathBuf> {
    let data_path = match BaseDirs::new() {
        None => bail!("Cannot find home directory"),
        Some(base_dirs) => base_dirs.data_dir().to_path_buf(),
    };
    let data_dir = data_path.join("shitternet");
    if !data_dir.exists() {
        fs::create_dir(&data_dir)?;
    }
    Ok(data_path.join(&data_dir).join(Path::new("up_down.log")))
}

fn get_log_file() -> Result<File> {
    let log_path = get_log_path()?;
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(log_path)?;

    Ok(file)
}

fn make_log_file_if_not_exists() -> Result<()> {
    if !get_log_path()?.exists() {
        let _ = get_log_file()?;
    }

    Ok(())
}
