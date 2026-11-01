# Vozbit

**Dicta en Windows y convierte tu voz en texto con modelos que se ejecutan localmente.** Vozbit está pensado para escribir en chats, documentos y editores de código.

[Descargar Vozbit para Windows x64](https://github.com/yordy-dev/vozbit/releases/latest) · [Ver todas las versiones](https://github.com/yordy-dev/vozbit/releases) · [Reportar un problema](https://github.com/yordy-dev/vozbit/issues)

## Funciones

- Transcripción local con **Whisper base** o **Parakeet Ultra**; puedes elegir el motor y el micrófono en la app.
- Atajo global configurable para iniciar y terminar un dictado. El atajo predeterminado es `Ctrl+Shift+Space`.
- Entrega del texto al campo activo, con opción de revisar o copiar el resultado desde Inicio.
- Historial local, diccionario personal y aprendizaje opcional de correcciones.
- Ventana compacta de estado y acceso desde la bandeja del sistema.

La transcripción puede equivocarse, especialmente con nombres, ruido o fórmulas. Revisa el resultado antes de usarlo en contextos importantes.

## Instalar y empezar

1. Descarga `Vozbit_<versión>_x64-setup.exe` desde [Releases](https://github.com/yordy-dev/vozbit/releases/latest) y ejecuta el instalador.
2. Abre Vozbit y selecciona el micrófono y el motor de transcripción.
3. Coloca el cursor en el campo donde quieres escribir.
4. Usa el atajo configurado. Al finalizar el dictado, Vozbit procesa el audio y entrega el texto según el modo de salida elegido.

El instalador incluye los motores, modelos locales y el instalador offline de WebView2. No se necesita conexión para instalar; descargar el instalador sí requiere Internet.

## Privacidad

- Whisper y Parakeet procesan el audio localmente; el dictado no se envía a un servicio remoto de transcripción.
- El historial guarda texto en el equipo y se puede desactivar en Configuración. El audio no se conserva en el historial.
- El aprendizaje de correcciones está desactivado hasta que lo actives. Si lo habilitas, Vozbit observa durante un período limitado el campo editable de destino para reconocer correcciones; no observa campos de contraseña.
- El envío al campo activo usa funciones locales de Windows. Si no puede confirmar el destino, conserva el resultado en Inicio para que puedas revisarlo y copiarlo.

## Compilar desde el código

Se requieren Node.js, Rust y los prerrequisitos de escritorio de Tauri para Windows.

Desde la raíz del repositorio, descarga y verifica los modelos grandes:

```powershell
.\scripts\fetch-models.ps1
```

Después compila el instalador:

```powershell
cd app
npm ci
npm run tauri -- build --bundles nsis
```

Los modelos de Whisper y Parakeet se distribuyen como assets de [Releases](https://github.com/yordy-dev/vozbit/releases), no en el historial de Git. `scripts/fetch-models.ps1` descarga las versiones fijadas y comprueba sus hashes SHA-256.

## Solución de problemas

- **No se oye el dictado:** revisa el micrófono elegido y los permisos de micrófono de Windows.
- **El atajo no responde:** verifica el atajo en Configuración y que otra app no esté usando la misma combinación.
- **El texto no aparece en el campo:** revisa el modo de entrega; el resultado permanece disponible en Inicio para copiarlo manualmente.
- **La descarga del modelo falla:** vuelve a ejecutar `fetch-models.ps1` con conexión a Internet; el script verifica el archivo antes de guardarlo.

## Componentes y licencia

Los motores y modelos de terceros conservan sus propias licencias y atribuciones, incluidas en `app/src-tauri/resources/THIRD_PARTY_LICENSES.txt` y en los recursos correspondientes. **La licencia de reutilización del código de Vozbit aún no está definida**; el repositorio es público, pero no concede por sí solo permiso para reutilizar el código.
