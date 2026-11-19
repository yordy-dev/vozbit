# Auditoría de interfaz y entrega de Vozbit 0.5.11

## Problema

Vozbit incorpora una nueva pantalla Inicio y cuatro apariencias, pero todavía no existe una comprobación trazable que relacione cada pantalla, estado y control con una evidencia visual y funcional. Sin esa auditoría, una entrega podría conservar controles obsoletos, romper un tema, incluir residuos del proyecto o publicar binarios que no correspondan al código.

## Objetivo y resultado esperado

Auditar el inventario definido abajo, conservar solo controles con una función verificable y publicar Vozbit 0.5.11 para Windows x64. La entrega queda completa cuando el commit, la etiqueta `v0.5.11`, las versiones del código, el ejecutable, el instalador, sus tamaños y sus SHA-256 coinciden en un manifiesto; además, la compilación final queda abierta para la prueba del usuario.

## Alcance verificable

- Inicio: reposo, grabación, procesamiento, resultado y error.
- Historial: vacío, búsqueda, elementos, favoritos, copia, eliminación y limpieza.
- Diccionario: alta manual, aprendizaje automático, búsqueda, prueba, activación y eliminación.
- Configuración: General, Modelo local, Texto y salida, Apariencia, Datos y app, Ayuda y acerca de.
- Ayuda, Acerca de, tutorial, bandeja e Indicador mini.
- Apariencias: Sistema, Claro, Oscuro y Contraste.

## No objetivos y contratos protegidos

- No rediseñar el Indicador mini ni cambiar sus estados grabando, procesando y error.
- No modificar el motor de transcripción, el seguimiento del cursor ni el aprendizaje de correcciones salvo un fallo demostrado por las pruebas de regresión.
- No eliminar recuperación, privacidad, diagnóstico, tutorial o ayuda por ser funciones de uso poco frecuente.
- No borrar datos personales existentes durante una actualización ni realizar pruebas destructivas contra el perfil real.
- No incluir modelos locales, bases de datos personales, secretos, artefactos temporales ni borradores en Git.

## Arquitectura afectada

- React (`App`, `SettingsPanel`, `HistoryPanel`, `DictionaryPanel`, `Help`, `Tutorial` y `Overlay`) presenta las pantallas y aplica el atributo `data-theme`.
- `main.tsx` carga las hojas globales `style.css`, `refinements.css`, `polish.css` y `redesign.css`; las hojas de componente completan Historial, Diccionario, Configuración, Tutorial e Indicador mini. El Indicador conserva sus archivos y contrato actuales.
- `api.ts` conecta React con Tauri; Rust (`lib.rs`, `storage.rs`) persiste `theme` y las demás preferencias en SQLite. Las actualizaciones mantienen el identificador de aplicación.
- Tauri/NSIS genera el ejecutable y el instalador Windows x64.
- No se introduce un servicio externo ni un cambio de arquitectura.

## Matriz de evidencia

| ID | Superficie/estado | Precondición y acción | Resultado esperado | Prueba y artefacto |
|---|---|---|---|---|
| UI-01 | Inicio en reposo | abrir la compilación final | atajo, micrófono, motor y último dictado visibles | smoke UI + `inicio-*.png` |
| UI-02 | Inicio grabando/procesando/error | ejecutar smoke del Indicador mini | estados nativos sin cambiar su contrato | smoke indicador + capturas |
| UI-03 | Inicio resultado | resultado disponible; copiar/guardar/expandir | acción observable y texto recuperable | pruebas Rust + smoke nativo |
| UI-04 | Historial vacío/listado | abrir Historial con perfil de prueba | vacío o elementos sin desbordar | smoke UI + `pantalla-historial.png` |
| UI-05 | Historial buscar/favorito/copiar/eliminar/limpiar | datos sembrados en perfil aislado | cada acción actualiza solo el dato esperado | pruebas de integración + smoke aislado |
| UI-06 | Diccionario alta/búsqueda/prueba | regla de prueba en perfil aislado | se guarda, encuentra y previsualiza | pruebas de aprendizaje + captura |
| UI-07 | Diccionario activar/eliminar/revisar | regla o sugerencia existente | cambia el estado o desaparece con confirmación | pruebas Rust/integración |
| UI-08 | Configuración General | abrir, guardar atajo y refrescar micrófonos | control invocable y respuesta visible | smoke de navegación + pruebas Rust |
| UI-09 | Modelo local | comprobar motores | estado real de Parakeet y Whisper | smoke nativo |
| UI-10 | Texto y salida | cambiar opciones reversibles | valor persistido y efecto cubierto por pruebas | pruebas Rust/integración |
| UI-11 | Apariencia | elegir `system`, `light`, `dark`, `contrast` | aplicación inmediata, selección y persistencia | smoke UI + `apariencia-*.png` |
| UI-12 | Datos y app | respaldo, modo privado, diagnóstico | no expone texto; operaciones seguras | smoke aislado de endurecimiento |
| UI-13 | Ayuda y Acerca de | navegar desde Configuración | contenido, versión y regreso correctos | smoke UI + capturas |
| UI-14 | Tutorial | abrir y recorrer bienvenida + cinco pasos | foco, tamaño y cierre correctos | smoke tutorial + captura |
| UI-15 | Indicador mini | oculto, grabando, procesando, error | no toma foco y conserva geometría/estados 0.5.10 | smoke indicador nativo |
| UI-16 | Bandeja | abrir menú | abrir Vozbit, dictar, último texto, diccionario, configuración, tutorial, ayuda y salir disponibles | inspección nativa |
| UI-17 | Instalación nueva | inspeccionar paquete y, si existe, VM/Sandbox desechable | no incluye DB ni textos de otro usuario | inventario NSIS + informe |
| UI-18 | Actualización 0.5.10→0.5.11 | perfil desechable verificado antes de sembrar datos | historial, diccionario, preferencias y correcciones iguales | prueba aislada o limitación explícita |

La ventana principal se comprueba en su tamaño nativo, sin desbordamiento horizontal, y todo control interactivo visible debe tener al menos 36 px de alto. Cada botón, enlace, selector o interruptor debe producir una acción observable, persistir el valor correspondiente o explicar su indisponibilidad. La prueba destructiva del binario Release queda prohibida en el perfil real: Windows Sandbox no está disponible en este equipo, de modo que UI-17/UI-18 se cubren mediante pruebas aisladas debug, compatibilidad de esquema, identidad estable e inventario del paquete, y esta limitación se declara en el informe.

## Puertas de aceptación

- `npm test` termina con código 0 y sin pruebas omitidas.
- `cargo fmt --check`, `cargo check`, `cargo test --locked -- --test-threads=1` y `npm audit --omit=dev --audit-level=high` terminan con código 0.
- Los smokes nativos de navegación, auditoría UI, tutorial e Indicador mini terminan con código 0.
- La matriz anterior tiene evidencia y no presenta desbordamiento horizontal ni controles visibles menores de 36 px.
- Sistema responde al modo claro/oscuro de Windows; Claro, Oscuro y Contraste se aplican inmediatamente, se guardan y reaparecen al reiniciar.
- El Indicador mini conserva los estados oculto, grabando, procesando y error de la línea base 0.5.10.
- Git permite únicamente fuente, pruebas, documentación curada e imágenes aprobadas; excluye `artifacts/`, bases SQLite, audio, pesos de modelos y `design_draft.md`. El instalador debe incluir los modelos locales declarados en Tauri; la Release permite instalador, ejecutable portátil, notas, manifiesto y SHA-256.
- El manifiesto relaciona rama `codex/scenario-hardening`, commit, etiqueta `v0.5.11`, `package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`, nombres, tamaños y SHA-256. README, nombre de binario y metadatos son controles adicionales.
- Una actualización conserva historial, diccionario, preferencias y correcciones; una instalación nueva no contiene registros creados por otro usuario.

## Regla de eliminación

Un elemento solo se elimina si no tiene importaciones ni referencias en TSX/Rust/configuración, no está registrado como recurso o ruta nativa, no es alcanzable en ejecución y el build y las pruebas continúan pasando. En 0.5.11 no se elimina ningún archivo: solo reglas CSS heredadas demostradas como inactivas. Se conservan explícitamente diagnóstico, creación de respaldo, recuperación de texto, privacidad, tutorial, ayuda y aprendizaje aunque su uso sea ocasional.

## Línea base y allowlist de cambios

- Commit base: `05f2ffd3a4a9dd84ba3b866b72f60f5d2e495f10`.
- Modificar únicamente: `.gitignore`, `README.md`, las cinco fuentes de versión, `storage.rs`, `App.tsx`, `SettingsPanel.tsx`, `Tutorial.tsx`, `types.ts`, `dictation.css`, `dictionary.css`, `polish.css`, `redesign.css`, `settings.css`, `style.css`, `tutorial.css` y `hardening-static.mjs`.
- Crear únicamente: `smoke-ui-audit-v0511.mjs`, documentos 46/47, informe EGM consolidado y cuatro capturas curadas.
- No borrar archivos. No versionar `artifacts/`, `design_draft.md`, SQLite, audio, `target/`, `dist/`, `node_modules/` ni pesos de modelos.

## Protocolo aislado

1. Las pruebas destructivas usan exclusivamente un build debug iniciado con `VOICETEXT_DATA_DIR=<ruta absoluta nueva dentro de artifacts>` y puerto CDP dedicado.
2. Antes de sembrar o borrar, el smoke exporta un diagnóstico y exige que su ruta y el respaldo comiencen por esa carpeta aislada.
3. `smoke-hardening-v059.mjs` nunca se ejecuta contra el binario Release ni sin el tercer argumento de carpeta aislada.
4. El Release solo recibe smokes de lectura/navegación sobre el perfil existente; no se borra, siembra ni modifica historial o diccionario personal.
5. Como Windows Sandbox no está disponible, UI-17/UI-18 se cierran con inventario NSIS, ausencia de SQLite empaquetado, identificador estable y pruebas de esquema/migración en debug; el informe declara que no se ejecutó una actualización destructiva del instalador en VM.

## Artefactos de distribución

- Instalador: `Vozbit_0.5.11_x64-setup.exe`, con recursos y modelos declarados por Tauri.
- Portátil: `Vozbit_0.5.11_x64-portable.zip`, compuesto por `voicetext-local.exe`, el árbol `resources/` y licencias; el EXE suelto no se publica como portátil.
- Evidencia: `Vozbit_0.5.11_evidencia-ui.zip`, informe, notas y `SHA256SUMS-v0.5.11.txt`.

## Evidencia durable

- El informe final, la matriz cerrada y el manifiesto quedan rastreados en `docs/46_AUDITORIA_UI_RELEASE_V0.5.11.md` y `docs/47_INFORME_RELEASE_V0.5.11.md`.
- Las capturas aprobadas de Inicio, Diccionario y Apariencia quedan en `docs/images/`; el juego completo de auditoría queda adjunto a la Release como archivo de evidencia.
- Los archivos generados locales permanecen en `artifacts/v0511-audit/` y no entran en Git.

## Autorización de publicación

La petición vigente del usuario autoriza terminar la auditoría, compilar una nueva versión y subir todo lo aprobado al repositorio. Esa instrucción constituye la aprobación para publicar rama, `main`, etiqueta y Release después de superar las puertas; no autoriza alterar el perfil personal ni publicar datos privados.

## Alternativas consideradas

1. **Conservar todo el legado:** reduce riesgo inmediato, pero mantiene CSS y archivos sin uso y dificulta la auditoría; se descarta.
2. **Reescribir toda la interfaz:** produciría una línea base limpia, pero ampliaría el riesgo y rompería el alcance aprobado; se descarta.
3. **Limpieza probada y acotada:** elimina únicamente residuos demostrados y conserva contratos funcionales; es la opción elegida.
4. **Publicar solo código:** no satisface la descarga del usuario; se publicarán código y Release como entregas separadas.

## Riesgos y mitigaciones

| Riesgo | Impacto | Mitigación | Limitación residual |
|---|---|---|---|
| Borrar una función poco visible | pérdida de recuperación o privacidad | regla de eliminación + smokes | una ruta dependiente del sistema puede requerir prueba manual |
| Romper contraste o geometría | interfaz ilegible | capturas de cuatro temas + medidas CDP | la percepción visual final la confirma el usuario |
| Dañar datos al probar | pérdida de historial/diccionario | perfil aislado; no borrar el perfil real | no se simula una VM Windows limpia si no está disponible |
| Publicar binarios incoherentes | descarga no reproducible | manifiesto, hashes y verificación de Release | binarios sin firma muestran advertencia de Windows |
| Incluir datos o modelos | privacidad y repositorio excesivo | inventario Git, reglas de exclusión y revisión de assets | se requiere repetir el control en futuras versiones |

## Detailed Implementation Plan

1. **Inventario:** relacionar UI-01–UI-18 con componentes, comandos y evidencia; cierre: ninguna fila sin método de comprobación o limitación explícita.
2. **Revisión EGM:** ejecutar las etapas aisladas y corregir todo Blocker y Major aplicable; cierre: informe consolidado sin Blockers y con cada Major resuelto o justificado.
3. **Limpieza probada:** aplicar exclusivamente la Regla de eliminación a estilos o archivos sin uso; cierre: diff recuperable, `rg` sin referencias y build/pruebas verdes, sin cambios en `Overlay.tsx` ni `overlay-refined.css`.
4. **Pruebas de código:** desde `app`, ejecutar `npm test` y `npm audit --omit=dev --audit-level=high`; desde `app/src-tauri`, usar `C:\Users\yodrf\.cargo\bin\cargo.exe` para `fmt --check`, `check` y `test --locked -- --test-threads=1`. Cierre: código 0, cero fallos/ignoradas y cero vulnerabilidades altas.
5. **Pruebas nativas y evidencia:** iniciar la aplicación con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9461`; desde `app`, ejecutar con `node tests/...` los smokes de navegación, auditoría UI, tutorial e indicador, pasando puerto y `artifacts/v0511-audit` cuando corresponda. Los controles destructivos se validan en debug mediante el Protocolo aislado; bandeja y acciones dependientes de Windows se inspeccionan nativamente. Cierre: cada UI-ID queda aprobado, respaldado por pruebas de backend/integración o marcado con la limitación explícita de UI-17/UI-18; no hay desbordamiento ni objetivos menores de 36 px.
6. **Compilación 0.5.11:** desde la raíz, ejecutar `pwsh -NoProfile -File scripts/fetch-models.ps1` (verifica SHA-256 y solo descarga si falta un modelo); desde `app`, ejecutar `npm ci` en un clon limpio o comprobar lockfile en el árbol actual y `npm run tauri -- build`. Cierre: `target/release/voicetext-local.exe`, `target/release/bundle/nsis/Vozbit_0.5.11_x64-setup.exe`, recursos y licencias presentes; crear y extraer el ZIP portátil en una carpeta nueva y verificar su inventario.
7. **Manifiesto y ejecución:** calcular tamaño y SHA-256 de cada asset, registrar commit/etiqueta/versiones/limitaciones en `docs/47_INFORME_RELEASE_V0.5.11.md`, abrir el ejecutable final y comprobar proceso responsivo, título y ventana; cierre: manifiesto completo y Vozbit abierto para prueba.
8. **Publicación verificada:** comprobar acceso mediante `git push --dry-run` y GitHub REST usando la credencial configurada sin imprimirla. Secuencia: commit → push de `codex/scenario-hardening` → verificación remota → fast-forward de `main` → etiqueta → Release draft → carga de nombres exactos → descarga y comparación de tamaños/SHA-256 → publicación. Ante cualquier fallo, mantener la Release como draft y no mover la siguiente referencia. Cierre: ramas y etiqueta apuntan al commit del manifiesto y la Release pública ofrece assets idénticos.

## Resultado

Pendiente de completar durante esta ejecución.
