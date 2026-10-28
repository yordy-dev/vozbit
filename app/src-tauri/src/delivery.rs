use serde::Serialize;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, GetGUIThreadInfo, GetWindowRect, GetWindowTextW, IsIconic, IsWindow, SetForegroundWindow, ShowWindow, GUITHREADINFO, SW_RESTORE};
use std::hash::{Hash, Hasher};
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, UIA_EditControlTypeId};

#[derive(Clone, Copy)]
pub struct DeliveryTarget {
    hwnd: usize,
    pid: u32,
    focus: usize,
    caret: usize,
    title_hash: Option<u64>,
    text_element: Option<u64>,
}
pub fn process_id(target: Option<DeliveryTarget>) -> Option<u32> {target.map(|t|t.pid)}
pub fn window_center(target: DeliveryTarget) -> Option<(i32, i32)> {
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(target.hwnd as _, &mut rect) } == 0 { return None; }
    Some((rect.left + (rect.right - rect.left) / 2, rect.top + (rect.bottom - rect.top) / 2))
}

fn gui_thread_info() -> Option<GUITHREADINFO> {
    let mut info: GUITHREADINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(0, &mut info) } != 0 { Some(info) } else { None }
}

fn title_hash(hwnd: usize) -> Option<u64> {
    let mut title = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd as _, title.as_mut_ptr(), title.len() as i32) };
    if length <= 0 { return None; }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title[..length as usize].hash(&mut hasher);
    Some(hasher.finish())
}

struct ComApartment;

impl Drop for ComApartment {
    fn drop(&mut self) { unsafe { CoUninitialize() }; }
}

struct TextFocusProbe {
    // Fields are dropped in declaration order: release UI Automation before COM.
    automation: IUIAutomation,
    _com: ComApartment,
}

impl TextFocusProbe {
    fn new() -> Option<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()? };
        let com = ComApartment;
        let automation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()? };
        Some(Self { automation, _com: com })
    }

    fn focused_edit_id(&self, pid: u32) -> Option<u64> {
        let element = unsafe { self.automation.GetFocusedElement().ok()? };
        if unsafe { element.CurrentProcessId().ok()? } != pid as i32
            || unsafe { element.CurrentControlType().ok()? } != UIA_EditControlTypeId
            || unsafe { element.CurrentIsPassword().ok()?.as_bool() } {
            return None;
        }
        let array = unsafe { element.GetRuntimeId().ok()? };
        let result = (|| {
            let first = unsafe { SafeArrayGetLBound(array, 1).ok()? };
            let last = unsafe { SafeArrayGetUBound(array, 1).ok()? };
            if last < first || last - first > 64 { return None; }
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            pid.hash(&mut hasher);
            for index in first..=last {
                let mut value = 0_i32;
                unsafe { SafeArrayGetElement(array, &index, &mut value as *mut _ as *mut _).ok()? };
                value.hash(&mut hasher);
            }
            Some(hasher.finish())
        })();
        let _ = unsafe { SafeArrayDestroy(array) };
        result
    }

    fn focused_password(&self, pid: u32) -> bool {
        let Ok(element) = (unsafe { self.automation.GetFocusedElement() }) else { return false; };
        (unsafe { element.CurrentProcessId().ok() }) == Some(pid as i32)
            && unsafe { element.CurrentIsPassword().ok() }.is_some_and(|value| value.as_bool())
    }
}

fn capture_text_target(probe: Option<&TextFocusProbe>) -> Option<DeliveryTarget> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() { return None; }
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 || pid == std::process::id() { return None; }
    let mut info: GUITHREADINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(thread, &mut info) } == 0 { return None; }
    let text_element = probe.and_then(|probe| probe.focused_edit_id(pid));
    if text_element.is_none() && probe.is_some_and(|probe| probe.focused_password(pid)) {
        return None;
    }
    if text_element.is_none() && info.hwndCaret.is_null() { return None; }
    if unsafe { GetForegroundWindow() } != hwnd { return None; }
    Some(DeliveryTarget {
        hwnd: hwnd as usize, pid, focus: info.hwndFocus as usize,
        caret: info.hwndCaret as usize, title_hash: title_hash(hwnd as usize), text_element,
    })
}

/// Keeps the latest *verified* text focus, including when another non-text
/// window becomes foreground while the user is speaking.
pub struct TargetTracker {
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<DeliveryTarget>>>,
    commands: mpsc::Sender<TrackerCommand>,
    worker: Option<JoinHandle<()>>,
}

enum TrackerCommand {
    Snapshot(mpsc::SyncSender<Option<DeliveryTarget>>),
    Verify { pid: u32, expected: Option<u64>, reply: mpsc::SyncSender<FocusVerification> },
}

enum FocusVerification { Matched, Changed, Password, Unavailable }

impl TargetTracker {
    pub fn start() -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let latest = Arc::new(Mutex::new(None));
        let (commands, requests) = mpsc::channel();
        let worker_stop = stop.clone();
        let worker_latest = latest.clone();
        let worker = std::thread::Builder::new().name("voicetext-text-focus".into()).spawn(move || {
            let probe = TextFocusProbe::new();
            while !worker_stop.load(Ordering::Acquire) {
                if let Some(target) = capture_text_target(probe.as_ref()) {
                    if let Ok(mut slot) = worker_latest.lock() { *slot = Some(target); }
                }
                match requests.recv_timeout(Duration::from_millis(90)) {
                    Ok(TrackerCommand::Snapshot(reply)) => {
                        let _ = reply.send(worker_latest.lock().ok().and_then(|slot| *slot));
                    }
                    Ok(TrackerCommand::Verify { pid, expected, reply }) => {
                        let result = match probe.as_ref() {
                            None => FocusVerification::Unavailable,
                            Some(probe) if probe.focused_password(pid) => FocusVerification::Password,
                            Some(probe) if expected.is_some() && probe.focused_edit_id(pid) == expected => FocusVerification::Matched,
                            Some(_) if expected.is_some() => FocusVerification::Changed,
                            Some(_) => FocusVerification::Matched,
                        };
                        let _ = reply.send(result);
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {},
                }
            }
        }).ok();
        Self { stop, latest, commands, worker }
    }

    pub fn latest(&self) -> Option<DeliveryTarget> {
        self.latest.lock().ok().and_then(|slot| *slot)
    }

    pub fn snapshot(&self) -> Option<DeliveryTarget> {
        let (reply, result) = mpsc::sync_channel(1);
        if self.commands.send(TrackerCommand::Snapshot(reply)).is_ok() {
            if let Ok(target) = result.recv_timeout(Duration::from_secs(2)) { return target; }
        }
        self.latest()
    }

    fn verify(&self, target: DeliveryTarget) -> FocusVerification {
        let (reply, result) = mpsc::sync_channel(1);
        if self.commands.send(TrackerCommand::Verify {
            pid: target.pid, expected: target.text_element, reply,
        }).is_err() { return FocusVerification::Unavailable; }
        result.recv_timeout(Duration::from_secs(2)).unwrap_or(FocusVerification::Unavailable)
    }
}

impl Drop for TargetTracker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

#[derive(Clone, Serialize)]
pub struct DeliveryStatus {
    pub(crate) sent: bool,
    pub(crate) reason: Option<String>,
}

pub fn skipped() -> DeliveryStatus {
    DeliveryStatus {
        sent: false,
        reason: None,
    }
}

pub fn capture_target() -> Option<DeliveryTarget> {
    // The handle is used only as an opaque identifier, never dereferenced.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 || pid == std::process::id() {
        return None;
    }
    let focus_info = gui_thread_info();
    Some(DeliveryTarget {
        hwnd: hwnd as usize,
        pid,
        focus: focus_info.as_ref().map_or(0, |info| info.hwndFocus as usize),
        caret: focus_info.as_ref().map_or(0, |info| info.hwndCaret as usize),
        title_hash: title_hash(hwnd as usize),
        text_element: None,
    })
}

fn manual(reason: &str) -> DeliveryStatus {
    DeliveryStatus {
        sent: false,
        reason: Some(reason.into()),
    }
}

fn unicode_event(unit: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn validate_target(target: DeliveryTarget) -> Result<(), &'static str> {
    if unsafe { IsWindow(target.hwnd as _) } == 0 {
        return Err("El campo original ya no está disponible; el texto quedó en Inicio");
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(target.hwnd as _, &mut pid) };
    if pid != target.pid {
        return Err("La ventana original cambió; no se insertó texto en otra aplicación");
    }
    if let Some(original_title) = target.title_hash {
        if title_hash(target.hwnd) != Some(original_title) {
            return Err("Cambió la página o conversación original; el texto quedó en Inicio");
        }
    }
    Ok(())
}

fn restore_target(target: DeliveryTarget, tracker: &TargetTracker) -> Result<(), &'static str> {
    validate_target(target)?;
    if unsafe { GetForegroundWindow() } as usize != target.hwnd {
        // Keyboard input only reaches the foreground window. Windows can deny activation;
        // in that case, leave the transcript available for manual copy.
        if target.focus == 0 {
            return Err("No se pudo verificar el campo original al cambiar de ventana; el texto quedó en Inicio");
        }
        if unsafe { IsIconic(target.hwnd as _) } != 0 {
            unsafe { ShowWindow(target.hwnd as _, SW_RESTORE) };
        }
        unsafe { SetForegroundWindow(target.hwnd as _) };
        let deadline = Instant::now() + Duration::from_millis(650);
        while unsafe { GetForegroundWindow() } as usize != target.hwnd && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    if unsafe { GetForegroundWindow() } as usize != target.hwnd {
        return Err("Windows no permitió volver al chat original; el texto quedó en Inicio");
    }
    validate_target(target)?;
    let focus_info = gui_thread_info();
    if target.focus != 0 && focus_info.as_ref().map_or(0, |info| info.hwndFocus as usize) != target.focus {
        return Err("Cambió el campo de texto original; el texto quedó en Inicio");
    }
    if target.caret != 0 && focus_info.as_ref().map_or(0, |info| info.hwndCaret as usize) != target.caret {
        return Err("Cambió el cursor de texto original; el texto quedó en Inicio");
    }
    match tracker.verify(target) {
        FocusVerification::Matched => {},
        FocusVerification::Changed => return Err("El cursor de texto ya no está en el campo elegido; el texto quedó en Inicio"),
        FocusVerification::Password => return Err("No se inserta dictado en campos de contraseña; el texto quedó en Inicio"),
        FocusVerification::Unavailable if target.text_element.is_some() =>
            return Err("No se pudo comprobar el campo de texto; el texto quedó en Inicio"),
        FocusVerification::Unavailable => {}, // Win32 caret/focus checks above remain authoritative.
    }
    Ok(())
}

pub fn deliver(tracker: Option<&TargetTracker>, target: Option<DeliveryTarget>, text: &str) -> DeliveryStatus {
    let Some(target) = target else {
        return manual(
            "No se identificó una ventana de destino externa; copia el texto manualmente",
        );
    };
    if text.is_empty() {
        return manual("No hay texto que enviar");
    }
    let units = text.encode_utf16().collect::<Vec<_>>();
    if units.len() > 10_000 {
        return manual("El texto es demasiado largo para el envío directo; cópialo manualmente");
    }
    for _ in 0..40 {
        if ![VK_CONTROL,VK_MENU,VK_SHIFT].into_iter().any(|k| (unsafe { GetAsyncKeyState(k as i32) } as u16 & 0x8000) != 0) { break; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let Some(tracker) = tracker else { return manual("No se pudo verificar el cursor de texto; el texto quedó en Inicio"); };
    if let Err(reason) = restore_target(target, tracker) { return manual(reason); }
    if [VK_CONTROL, VK_MENU, VK_SHIFT]
        .into_iter()
        .any(|key| (unsafe { GetAsyncKeyState(key as i32) } as u16 & 0x8000) != 0)
    {
        return manual("Hay teclas modificadoras presionadas; copia el texto manualmente");
    }
    let mut events = Vec::with_capacity(units.len() * 2);
    for unit in units {
        events.push(unicode_event(unit, false));
        events.push(unicode_event(unit, true));
    }
    let sent = unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent != events.len() as u32 {
        return manual("Windows no aceptó todas las teclas. El texto sigue disponible para copia manual; un destino elevado puede bloquear el envío");
    }
    DeliveryStatus {
        sent: true,
        reason: None,
    }
}
