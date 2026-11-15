# Vozbit 0.5.10 — aprendizaje local de correcciones

## Comportamiento

1. Vozbit inserta el dictado y observa durante un máximo de 30 segundos únicamente ese mismo campo.
2. Cuando sustituyes una palabra y la edición permanece estable aproximadamente medio segundo, compara el texto original con el corregido.
3. Una sustitución segura de una sola palabra guarda dos datos locales: la palabra correcta visible en Diccionario y la variante equivocada interna para corregirla en futuros dictados.
4. Windows muestra `Palabra aprendida: «…»` y la entrada puede desactivarse o eliminarse desde Diccionario.
5. Frases, números aislados, URLs y cambios inseguros no se aprenden automáticamente; pueden aparecer en **Por revisar**.

No requiere doble clic, cuenta, servidor, API key ni modelo de IA. No reentrena Parakeet o Whisper: aplica una corrección exacta comprobada después de la transcripción y utiliza el término correcto como vocabulario cuando el motor lo admite.

## Cómo usarlo

1. Activa **Configuración → Texto y salida → Aprender de mis correcciones**.
2. Conserva **Escribir en el campo activo** como modo de entrega.
3. Dicta y espera a que aparezca el texto.
4. Corrige la palabra equivocada en ese mismo campo antes de 30 segundos.
5. Espera la notificación y comprueba la palabra en **Diccionario**.

## Privacidad

- Audio, texto, comparación y aprendizaje permanecen en el equipo.
- No se envían correcciones a Groq, OpenAI ni ningún proveedor remoto.
- El modo privado desactiva el observador.
- Los campos marcados como contraseña nunca se observan.

## Verificación completada

- 32 pruebas Rust: aprobadas.
- Compilación `cargo check`: aprobada.
- Compilación TypeScript/Vite y 20 controles estáticos: aprobados.
- 18 pruebas del comparador de correcciones: aprobadas.
- Prueba integral Windows UI Automation: 6 correcciones detectadas dentro de un párrafo largo.
- Persistencia en SQLite, corrección posterior por alias y conservación de palabras manuales: comprobadas.
- Exclusión de frases y cambios inseguros del aprendizaje automático: comprobada.

## Entregables locales

- Ejecutable: `app/src-tauri/target/release/voicetext-local.exe`
  - SHA-256: `9312C9E746EB327D6DCEBD0A69DAF018CF3A457ECD81760C802AF7A5ECC557A4`
- Instalador: `app/src-tauri/target/release/bundle/nsis/Vozbit_0.5.10_x64-setup.exe`
  - SHA-256: `19E2FA62E1BA9C348E38B823D1BD365377BB7F5BC7FE4CA4CCCD89344112A846`

La única referencia binaria a la configuración del juez remoto pertenece a una migración que elimina esas claves antiguas. No existe un cliente, comando ni interfaz activa para aprendizaje remoto.
