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
use directories::BaseDirs;
use ping::ping;
use trayicon::{MenuBuilder, TrayIconBuilder};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
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

fn get_log_path() -> Result<PathBuf> {
    let data_path = match BaseDirs::new() {
        None => bail!("Could not find home directory"),
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

fn make_log_file() -> Result<()> {
    let _ = get_log_file()?;
    Ok(())
}

fn spawn_worker_thread(message_buf: Arc<Mutex<Vec<u8>>>) {
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
                    let mut message_buf = message_buf.lock().unwrap();
                    let message = "Shitternet came back up, hallelujah!\n";
                    message_buf.extend_from_slice(message.as_bytes());
                    messages_length += message.len();
                }
                (Up, Down) => {
                    let mut message_buf = message_buf.lock().unwrap();
                    let message = "Shitternet be shiddin (fard)!\n";
                    message_buf.extend_from_slice(message.as_bytes());
                    messages_length += message.len();
                }
                (Up, Up) => {
                    let mut message_buf = message_buf.lock().unwrap();
                    let message = "All good\n";
                    message_buf.extend_from_slice(message.as_bytes());
                    messages_length += message.len();
                }
                _ => {}
            }

            last_status = cur_status;

            // Dump every
            if messages_length >= 5 * KB {
                let mut log = get_log_file().unwrap();

                // If we can't get lock, it's because main loop is dumping contents, so it's fine
                if let Ok(mut m) = message_buf.try_lock() {
                    log.write_all(m.as_slice()).unwrap();
                    m.clear();
                }

                // But we should clear messages length either way
                // because other thread cleared messages or we did
                messages_length = 0;
            }

            thread::sleep(Duration::from_secs(30));
        }
    });
}

fn dump_messages(messages: &Arc<Mutex<Vec<u8>>>) {
    let mut log = get_log_file().unwrap();

    // Can't lock? Rick will be fine with old values
    if let Ok(mut m) = messages.try_lock() {
        log.write_all(m.as_slice()).unwrap();
        m.clear();
    }
}

fn main() -> Result<()> {
    eprintln!("Log file path: {:#?}", get_log_path()?);

    make_log_file()?;

    // Get the event loop
    let event_loop = EventLoop::<Events>::with_user_event();

    // Make an invisible window as the host for our app
    let my_app_window = WindowBuilder::new()
        .with_visible(false)
        .build(&event_loop)?;

    // Get a proxy for the event loop to give to our tray icon as a way to send events
    let proxy = event_loop.create_proxy();

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

    // Allocate a thread-safe storage buffer for messages to log
    let messages = Arc::new(Mutex::new(Vec::<u8>::new()));

    // Spawn a thread to ping network status
    spawn_worker_thread(Arc::clone(&messages));

    let log_path = get_log_path()?;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Move into here so it is cleaned up on exit
        let _ = tray_icon;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == my_app_window.id() => *control_flow = ControlFlow::Exit,

            Event::UserEvent(e) => match e {
                Events::OpenLog => {
                    dump_messages(&messages);
                    if let Err(e) = open::that(&log_path) {
                        eprintln!("Failed: {}", e)
                    } else {
                        eprintln!("Succeeded")
                    }
                }
                Events::Exit => *control_flow = ControlFlow::Exit,
                Events::ClickTrayIcon => (),
            },
            _ => (),
        }
    });
}
