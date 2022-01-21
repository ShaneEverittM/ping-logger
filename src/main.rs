#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{
    fs::{create_dir, File, OpenOptions},
    io::Write,
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{bail, Result};
use cached::proc_macro::once;
use directories::BaseDirs;
use ping::ping;
use trayicon::{MenuBuilder, TrayIcon, TrayIconBuilder};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::WindowBuilder,
};

const KB: usize = 1024;

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
    eprintln!("Log file path: {:#?}", get_log_path()?);

    make_log_file_if_not_exists()?;

    // Allocate a thread-safe storage buffer for messages to log
    let messages = Arc::new(Mutex::new(Vec::<u8>::new()));

    // Get the event loop
    let event_loop = EventLoop::<Events>::with_user_event();

    // Start pinging
    spawn_worker_thread(Arc::clone(&messages));

    // Let 'er rip
    start_event_loop(messages, event_loop)?;

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

fn spawn_worker_thread(messages: Arc<Mutex<Vec<u8>>>) {
    thread::spawn(move || {
        use Status::{Down, Up};
        let mut last_status: Status = Up;
        let mut cur_status: Status;

        let mut messages_length = 0;
        loop {
            cur_status = ping(
                IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
                None,
                None,
                None,
                None,
                None,
            )
                .into();

            match (last_status, cur_status) {
                (Down, Up) => {
                    let mut message_buf = messages.lock().unwrap();
                    let message = "Shitternet came back up, hallelujah!\n";
                    message_buf.extend_from_slice(message.as_bytes());
                    messages_length += message.len();
                }
                (Up, Down) => {
                    let mut message_buf = messages.lock().unwrap();
                    let message = "Shitternet be shiddin (fard)!\n";
                    message_buf.extend_from_slice(message.as_bytes());
                    messages_length += message.len();
                }
                (Up, Up) => {
                    let mut message_buf = messages.lock().unwrap();
                    let message = "All good\n";
                    message_buf.extend_from_slice(message.as_bytes());
                    messages_length += message.len();
                }
                _ => {}
            }

            last_status = cur_status;

            // Dump every
            if messages_length >= 5 * KB {
                try_dump_messages(&messages);

                // We should clear messages length either way
                // because other thread cleared messages or we did
                messages_length = 0;
            }

            thread::sleep(Duration::from_secs(30));
        }
    });
}

fn start_event_loop(messages: Arc<Mutex<Vec<u8>>>, event_loop: EventLoop<Events>) -> Result<()> {
    let log_path = get_log_path()?;

    // Make an invisible window as the host for our app
    let my_app_window = WindowBuilder::new()
        .with_visible(false)
        .build(&event_loop)?;

    // Make our tray
    let tray = make_tray(event_loop.create_proxy())?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Move into here so it is cleaned up on exit
        let _ = tray;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == my_app_window.id() => *control_flow = ControlFlow::Exit,

            Event::UserEvent(e) => match e {
                Events::OpenLog => {
                    try_dump_messages(&messages);
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

#[once(result = true)]
fn get_log_path() -> Result<PathBuf> {
    let data_path = match BaseDirs::new() {
        None => bail!("Cannot find home directory"),
        Some(base_dirs) => base_dirs.data_dir().to_path_buf(),
    };
    let data_dir = data_path.join("shitternet");
    if !data_dir.exists() {
        create_dir(&data_dir)?;
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

fn try_dump_messages(messages: &Arc<Mutex<Vec<u8>>>) {
    let mut log = get_log_file().unwrap();

    // Can't lock? Rick will be fine with old values
    if let Ok(mut m) = messages.try_lock() {
        log.write_all(m.as_slice()).unwrap();
        m.clear();
    }
}
