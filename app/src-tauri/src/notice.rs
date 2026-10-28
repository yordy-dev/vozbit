use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn show(app: &AppHandle, message: &str) {
    if let Err(error) = app
        .notification()
        .builder()
        .title("Vozbit")
        .body(message)
        .show()
    {
        eprintln!("No se pudo mostrar la notificación de Windows: {error}");
    }
}
