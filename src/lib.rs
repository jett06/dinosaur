#![windows_subsystem = "windows"]
mod consts;

use crate::consts::*;
use betrayer::{
    winit::WinitTrayIconBuilderExt,
    Icon,
    Menu,
    MenuItem,
    TrayError,
    TrayEvent,
    TrayIcon,
    TrayIconBuilder,
};
use png::Decoder;
use std::{
    io::Cursor,
    sync::{
        atomic::{
            AtomicBool,
            Ordering,
        },
        Arc,
    },
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{
        BOOL,
        HINSTANCE,
        TRUE,
    },
    System::SystemServices::{
        DLL_PROCESS_ATTACH,
        DLL_PROCESS_DETACH,
    },
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{
        ActiveEventLoop,
        ControlFlow,
        EventLoop,
    },
    window::WindowId,
};
use winsafe::{
    co,
    guard::CloseHandleGuard,
    prelude::*,
    SysResult,
    HPROCESS,
    HPROCESSLIST,
    HWND,
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Signal {
    Start,
    Stop,
    Quit,
}

struct App {
    tray: TrayIcon<Signal>,
    killer_running: Arc<AtomicBool>,
    should_exit: Arc<AtomicBool>,
    killer_thread: Option<thread::JoinHandle<()>>,
}

impl App {
    fn new_with(tray: TrayIcon<Signal>) -> Self {
        let killer_running = Arc::new(AtomicBool::new(true));
        let should_exit = Arc::new(AtomicBool::new(false));
        let killer_thread = Some(spawn_killer_thread(
            Arc::clone(&killer_running),
            Arc::clone(&should_exit),
        ));

        Self {
            tray,
            killer_running,
            should_exit,
            killer_thread,
        }
    }
}

impl ApplicationHandler<TrayEvent<Signal>> for App {
    fn resumed(&mut self, _: &ActiveEventLoop) {}
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TrayEvent<Signal>) {
        if let TrayEvent::Menu(signal) = event {
            match signal {
                Signal::Start => {
                    if self.killer_running.load(Ordering::SeqCst) {
                        HWND::NULL
                            .MessageBox("Already started.", APP_TITLE, co::MB::ICONINFORMATION)
                            .unwrap();
                    } else {
                        self.killer_running.store(true, Ordering::SeqCst);
                        HWND::NULL
                            .MessageBox("Started!", APP_TITLE, co::MB::ICONINFORMATION)
                            .unwrap();
                    }

                    self.tray.set_tooltip("DyKnow killer's running!");
                }
                Signal::Stop => {
                    if self.killer_running.load(Ordering::SeqCst) {
                        self.killer_running.store(false, Ordering::SeqCst);
                        HWND::NULL
                            .MessageBox("Stopped!", APP_TITLE, co::MB::ICONINFORMATION)
                            .unwrap();
                    } else {
                        HWND::NULL
                            .MessageBox("Already stopped.", APP_TITLE, co::MB::ICONINFORMATION)
                            .unwrap();
                    }

                    self.tray.set_tooltip("DyKnow killer is stopped.");
                }
                Signal::Quit => {
                    self.should_exit.store(true, Ordering::SeqCst);
                    if let Some(killer_thread) = self.killer_thread.take() {
                        if let Err(e) = killer_thread.join() {
                            HWND::NULL
                                .MessageBox(
                                    format!("Killer thread panicked! Error: {:#?}", e).as_str(),
                                    APP_TITLE,
                                    co::MB::ICONINFORMATION,
                                )
                                .unwrap();
                        }
                    }

                    event_loop.exit();
                }
            }
        }
    }
}

fn spawn_killer_thread(
    killer_running: Arc<AtomicBool>, should_exit: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        if killer_running.load(Ordering::SeqCst) {
            if let Ok(mut snapshot) =
                HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
            {
                _ = kill_processes_with_path_containing_selector(
                    PROCESS_KILL_SELECTOR,
                    &mut snapshot,
                );
            }
        }

        thread::sleep(Duration::from_millis(LOOP_INTERVAL_MILLIS));

        if should_exit.load(Ordering::SeqCst) {
            break;
        }
    })
}

fn kill_processes_with_path_containing_selector(
    kill_selector: &str, snapshot: &mut CloseHandleGuard<HPROCESSLIST>,
) -> SysResult<()> {
    let kill_selector_lowercase = kill_selector.to_lowercase();

    for process_entry in snapshot.iter_processes().filter_map(Result::ok) {
        if let Ok(process_info_handle) = HPROCESS::OpenProcess(
            co::PROCESS::QUERY_INFORMATION | co::PROCESS::VM_READ,
            false,
            process_entry.th32ProcessID,
        ) {
            if let Ok(process_path) =
                process_info_handle.QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            {
                if process_path
                    .as_str()
                    .to_lowercase()
                    .contains(&kill_selector_lowercase)
                {
                    if let Ok(process_kill_handle) = HPROCESS::OpenProcess(
                        co::PROCESS::TERMINATE,
                        false,
                        process_entry.th32ProcessID,
                    ) {
                        process_kill_handle.TerminateProcess(MAX_TERMINATION)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn app_icon() -> Result<Icon, TrayError> {
    let decoder = Decoder::new(Cursor::new(APP_ICON_PNG));
    let mut reader = decoder
        .read_info()
        .map_err(|e| TrayError::custom(e.to_string()))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| TrayError::custom(e.to_string()))?;

    Icon::from_rgba(buf, info.width, info.height)
}

#[no_mangle]
pub extern "system" fn DllMain(_: HINSTANCE, call_type: u32, _: *mut ()) -> BOOL {
    match call_type {
        DLL_PROCESS_ATTACH => {
            let event_loop = EventLoop::with_user_event().build().unwrap();
            let tray = TrayIconBuilder::new()
                .with_icon(app_icon().unwrap())
                .with_tooltip(APP_TITLE)
                .with_menu(Menu::new([
                    MenuItem::button("Start", Signal::Start),
                    MenuItem::button("Stop", Signal::Stop),
                    MenuItem::separator(),
                    MenuItem::button("Quit", Signal::Quit),
                ]))
                .build_event_loop(&event_loop, Some)
                .unwrap();
            let mut app = App::new_with(tray);

            event_loop.set_control_flow(ControlFlow::Wait);
            event_loop.run_app(&mut app).unwrap();
        }
        DLL_PROCESS_DETACH => {}
        _ => (),
    }
    TRUE
}
