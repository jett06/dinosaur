#![windows_subsystem = "windows"]
mod consts;

use crate::consts::*;
use betrayer::{
    winit::WinitTrayIconBuilderExt,
    Icon,
    Menu,
    MenuItem,
    TrayEvent,
    TrayIcon,
    TrayIconBuilder,
};
use std::{
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
    _tray: TrayIcon<Signal>,
    killer_running: Arc<AtomicBool>,
}

impl ApplicationHandler<TrayEvent<Signal>> for App {
    fn resumed(&mut self, _: &ActiveEventLoop) {}
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: TrayEvent<Signal>) {
        if let TrayEvent::Menu(signal) = event {
            match signal {
                Signal::Start => {
                    if !self.killer_running.load(Ordering::SeqCst) {
                        self.killer_running.store(true, Ordering::SeqCst);
                        let killer_running = Arc::clone(&self.killer_running);
                        thread::spawn(move || {
                            while killer_running.load(Ordering::SeqCst) {
                                if let Ok(mut snapshot) = HPROCESSLIST::CreateToolhelp32Snapshot(
                                    co::TH32CS::SNAPPROCESS,
                                    None,
                                ) {
                                    _ = kill_processes_with_path_containing_selector(
                                        PROCESS_KILL_SELECTOR,
                                        &mut snapshot,
                                    );
                                }
                                // In Rust, empty loops use a ton of CPU - this `sleep` call helps
                                // avoid wasteful CPU usage :)
                                thread::sleep(Duration::from_millis(LOOP_INTERVAL_MILLIS));
                            }
                        });
                        HWND::NULL
                            .MessageBox("Started!", APP_TITLE, co::MB::ICONINFORMATION)
                            .unwrap();
                    }
                }
                Signal::Stop => {
                    if self.killer_running.load(Ordering::SeqCst) {
                        self.killer_running.store(false, Ordering::SeqCst);
                        HWND::NULL
                            .MessageBox("Stopped!", APP_TITLE, co::MB::ICONINFORMATION)
                            .unwrap();
                    }
                }
                Signal::Quit => event_loop.exit(),
            }
        }
    }
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
                        process_kill_handle.TerminateProcess(9)?;
                    }
                }
            }
        }
    }

    Ok(())
}

#[no_mangle]
pub extern "system" fn DllMain(_: HINSTANCE, call_type: u32, _: *mut ()) -> BOOL {
    match call_type {
        DLL_PROCESS_ATTACH => {
            let event_loop = EventLoop::with_user_event().build().unwrap();
            let tray = TrayIconBuilder::new()
                .with_icon(Icon::from_rgba(vec![255u8; 32 * 32 * 4], 32, 32).unwrap())
                .with_tooltip(APP_TITLE)
                .with_menu(Menu::new([
                    MenuItem::button("Start", Signal::Start),
                    MenuItem::button("Stop", Signal::Stop),
                    MenuItem::separator(),
                    MenuItem::button("Quit", Signal::Quit),
                ]))
                .build_event_loop(&event_loop, Some)
                .unwrap();
            event_loop.set_control_flow(ControlFlow::Wait);
            event_loop
                .run_app(&mut App {
                    _tray: tray,
                    killer_running: Arc::new(AtomicBool::new(false)),
                })
                .unwrap();
        }
        DLL_PROCESS_DETACH => {}
        _ => (),
    }
    TRUE
}
