# Vozbit

Aplicación de dictado para Windows x64. Captura voz y la transcribe en el equipo con Whisper o Parakeet; el audio no se envía a un servicio de transcripción remoto.

## Descargar

Abre [Releases](https://github.com/yordy-dev/vozbit/releases/latest) y descarga `Vozbit_<versión>_x64-setup.exe`. El instalador incluye los motores, modelos locales y WebView2 sin requerir conexión durante la instalación. Consulta las notas de cada versión y verifica el SHA-256 publicado junto al instalador.

## Código fuente

- `app/src/`: interfaz React y TypeScript.
- `app/src-tauri/src/`: aplicación de escritorio y backend Rust/Tauri.
- `app/src-tauri/resources/`: motores, componentes de terceros y avisos de licencia.
- `scripts/fetch-models.ps1`: descarga y verifica los modelos grandes necesarios para compilar desde el código.

Los pesos de Whisper base y Parakeet Ultra se publican como assets de Releases, no dentro del historial Git. Para preparar un clon antes de compilar, ejecuta desde la raíz:

```powershell
.\scripts\fetch-models.ps1
cd app
npm ci
npm run tauri -- build --bundles nsis
```

Requisitos de desarrollo: Node.js, Rust y los prerrequisitos de escritorio de Tauri para Windows. La app de Release está lista para instalar sin estos requisitos.

## Privacidad y licencias

El dictado usa motores locales. La aplicación también puede comunicarse con otros procesos locales de Windows para entregar texto al campo enfocado. Revisa las opciones de privacidad y aprendizaje en la app. Los avisos de componentes de terceros están en `app/src-tauri/resources/THIRD_PARTY_LICENSES.txt`.

La licencia de reutilización del código de Vozbit no está especificada todavía; publicar el repositorio permite consultar el código, pero no concede por sí solo permisos adicionales de reutilización.
