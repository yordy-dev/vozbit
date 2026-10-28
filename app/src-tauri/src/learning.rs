use std::{io::{BufRead, BufReader, Write}, process::{Command, Stdio}, sync::{atomic::{AtomicU64, Ordering}, mpsc}, time::{Duration, Instant}};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use tauri::{AppHandle,Manager,Emitter};
use crate::storage;
static GENERATION: AtomicU64 = AtomicU64::new(0);
pub fn stop() { GENERATION.fetch_add(1,Ordering::Relaxed); }
pub fn observe(app: &AppHandle, pid:u32, text:&str) {
    launch(app,pid,text,false);
}
pub fn selected_word(app:&AppHandle) {
    if let Some(pid)=crate::delivery::process_id(crate::delivery::capture_target()) {launch(app,pid,"",true);}
}
fn launch(app: &AppHandle, pid:u32, text:&str, selection:bool) {
    stop();
    if !selection && (storage::read_setting(app,"learn_corrections").ok().flatten().as_deref()!=Some("true")
        || storage::read_setting(app,"learning_consent_v048").ok().flatten().as_deref()!=Some("true")) { return; }
    let generation=GENERATION.load(Ordering::Relaxed);
    let app=app.clone(); let text=text.to_string();
    std::thread::spawn(move || {
        let Ok(path)=app.path().resolve("resources/observe-correction.ps1",tauri::path::BaseDirectory::Resource) else{return};
        let Some(system_root)=std::env::var_os("SystemRoot") else{return};
        let powershell=PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        if !powershell.is_file() { return; }
        let Ok(mut child)=Command::new(powershell).args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File"]).arg(path)
            .creation_flags(0x08000000).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else{return};
        if let Some(mut input)=child.stdin.take() { let _=writeln!(input,"{}",serde_json::json!({"pid":pid,"owner":std::process::id(),"text":text,"selection":selection})); }
        let Some(output) = child.stdout.take() else { let _=child.kill(); let _=child.wait(); return };
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                match line { Ok(line) => { if sender.send(line).is_err() { break } }, Err(_) => break }
            }
        });
        let started=Instant::now();
        let mut observed = 0_usize;
        loop {
            while let Ok(line) = receiver.try_recv() {
                if observed < 20 && apply_candidate(&app, &line, selection, observed == 0) { observed += 1; }
            }
            if GENERATION.load(Ordering::Relaxed)!=generation || started.elapsed()>Duration::from_secs(40) {
                let _=child.kill(); let _=child.wait(); let _=reader.join(); return;
            }
            match child.try_wait() { Ok(Some(_))=>break,Err(_)=>{let _=child.kill();let _=child.wait();let _=reader.join();return},_=>std::thread::sleep(Duration::from_millis(100)) }
        }
        let _=child.wait();
        let _=reader.join();
        while let Ok(line) = receiver.try_recv() {
            if observed < 20 && apply_candidate(&app, &line, selection, observed == 0) { observed += 1; }
        }
        if selection && observed == 0 { crate::notice::show(&app,"No se pudo leer la selección. Agrégala desde Diccionario."); }
    });
}
#[derive(serde::Deserialize)] struct Candidate {term:String,replacement:String}

fn apply_candidate(app: &AppHandle, line: &str, selection: bool, notify: bool) -> bool {
    let Ok(candidate) = serde_json::from_str::<Candidate>(line) else { return false };
    if selection {
        let result=tauri::async_runtime::block_on(storage::dictionary_upsert(app.clone(),candidate.term,candidate.replacement));
        if result.is_ok(){let _=app.emit("learning://updated",());crate::notice::show(app,"Palabra seleccionada agregada al diccionario");}
        else {crate::notice::show(app,"Selecciona una palabra o expresión de hasta 80 caracteres");}
        return result.is_ok();
    }
    if let Ok(learned)=storage::save_suggestion(app,&candidate.term,&candidate.replacement) {
        let _=app.emit("learning://updated",());
        if notify { crate::notice::show(app,if learned {"Corrección aprendida. Puedes editarla o deshacerla en Diccionario."} else {"Corrección detectada. Revísala en Diccionario."}); }
        return true;
    }
    false
}
