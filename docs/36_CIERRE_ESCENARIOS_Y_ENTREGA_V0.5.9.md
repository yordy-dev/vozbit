# Vozbit 0.5.9 — cierre de endurecimiento

## Resultado

La auditoría de 500 escenarios se trató como una matriz de riesgos, no como 500 defectos confirmados. Esta versión aplica controles transversales a los problemas reproducibles y comprobables dentro del proyecto. Los casos que dependen de otro equipo, hardware, permisos de Windows o aplicaciones externas quedan explícitamente separados; no se presentan como verificados sin evidencia.

## Cambios implementados

- Entrega de texto: validación por UI Automation del mismo control editable, soporte para controles `Edit` y `Document`, rechazo de contraseñas, códigos de error estables y mensajes específicos para destino cerrado, cambiado, no editable, bloqueado o con modificadores pulsados.
- Privacidad: modo privado real; no guarda historial, no aprende correcciones y elimina/no conserva el último audio para reprocesar.
- Datos: limpieza confirmada de historial (con o sin favoritos), respaldo JSON local de configuración, historial, diccionario y sugerencias, y comprobación SQLite `quick_check`.
- Diagnóstico: verifica integridad de la base, escritura en datos/caché, arquitectura, CPU lógica y limpieza de temporales antiguos sin incluir audio ni texto.
- Recursos: comprueba espacio temporal antes de grabar o reprocesar; el diagnóstico muestra versión de Windows, RAM disponible, espacio de datos/caché y capacidades CPU. Puede exportar un JSON técnico que no contiene audio ni transcripciones.
- Ciclo de vida: cierre ordenado de grabación, transcripción, aprendizaje y procesos auxiliares tanto desde bandeja como al terminar la aplicación.
- Aprendizaje: tiempo real alineado a 30 segundos, bloqueo en modo privado y comprobación SHA-256 de los scripts antes de ejecutarlos.
- Audio/modelos: limpieza de WAV/temporales obsoletos, cancelación limpia y conservación de las verificaciones SHA-256 de Parakeet, Whisper y VAD.
- UX: errores accionables al faltar modelo/micrófono, reenumeración al recuperar foco, estado de privacidad coherente, historial eficiente hasta 500 entradas y confirmación doble para limpiezas destructivas.
- Pruebas: las pruebas de WebView aceptan tanto la URL empacada como la URL de desarrollo, evitando falsos negativos del entorno.

## Evidencia ejecutada

- `npm test`: compilación TypeScript/Vite aprobada, 20 controles estáticos y 9 pruebas de correcciones.
- `cargo test -- --test-threads=1`: 28 pruebas del backend y 14 del formateador; 42 aprobadas, 0 fallidas.
- WebView nativa aislada:
  - tutorial completo de cinco pasos, tarjetas dentro de pantalla y sin solapar el objetivo;
  - seis categorías de configuración, sin desbordamiento horizontal;
  - indicador oculto en reposo y estados grabación, procesamiento, error y cancelación;
  - Parakeet, Whisper y detectores VAD íntegros;
  - atajo global registrado;
  - diagnóstico local correcto, modo privado bloqueando historial, audio temporal descartado, respaldo creado y limpieza ejecutada.
- Prueba destructiva repetida en perfil de depuración aislado y verificado por ruta: se creó 1 entrada, el respaldo incluyó 1 entrada, la limpieza eliminó exactamente 1 y la búsqueda posterior devolvió 0.
- Diagnóstico aislado: base/datos/caché/RAM/disco en estado correcto; exportación guardada dentro del perfil aislado.
- `npm audit --omit=dev --audit-level=high`: 0 vulnerabilidades reportadas.

## Incidente de QA y recuperación

La primera ejecución de la prueba de endurecimiento sobre el binario `release` asumió erróneamente que `VOICETEXT_DATA_DIR` también aislaba versiones de producción; el código solo admite esa variable en compilaciones de depuración. La prueba creó primero un respaldo y después eliminó 20 entradas no favoritas del perfil normal.

Se detuvo la aplicación, se conservó una copia SQLite previa a la recuperación y se restauraron las 20 entradas desde `vozbit-backup-1793599381273.json`. La comparación SQL final obtuvo `backup=20`, `actual=20`, `missing=0`, `extra=0`, seguida de `PRAGMA quick_check(1) = ok`.

La prueba quedó corregida: respaldo y limpieza solo se ejecutan si recibe explícitamente una carpeta aislada y el archivo creado pertenece a esa carpeta. Dos controles estáticos impiden retirar esa barrera por accidente. En una prueba de producción sin carpeta aislada esas acciones destructivas se omiten.

La repetición final sobre producción confirmó `isolatedDataVerified=false`, `backup=null` y `clearedNonFavorites=null`; el historial normal permaneció en 20 entradas, la cantidad de respaldos no cambió y `quick_check` siguió en `ok`.

## Artefactos finales

- Ejecutable: `voicetext-local.exe` — 16 806 912 bytes — SHA-256 `615C99D5C64BC9435134FE416F013BE895BCF5F20DC4B4587E70EB6106DFBDB5`.
- Instalador: `Vozbit_0.5.9_x64-setup.exe` — 999 932 305 bytes — SHA-256 `3E780C6D87357723DEAA512643ECF7BA381774F68C97B79BE23E27A796128986`.
- Ambos informan producto Vozbit, versión 0.5.9 y estado Authenticode `NotSigned`.
- Smoke final del binario de producción: seis categorías, sin desbordamiento, ambos motores/VAD íntegros, atajo registrado, indicador oculto en reposo y estados grabación/procesamiento/error/cancelación correctos. La muestra automática final no contenía voz suficiente, por lo que validó la ruta de error de audio silencioso; no sustituye una prueba hablada del usuario.

## Límites que no deben confundirse con errores resueltos

- Falta una matriz física en Windows 10/11, distintas escalas DPI, varios monitores, Bluetooth, RDP/Citrix y hardware lento. Una sola máquina no demuestra esas combinaciones.
- La inserción depende de las políticas de foco, nivel de integridad y accesibilidad de la aplicación destino. Vozbit ahora evita destinos dudosos y conserva el resultado, pero Windows no permite garantizar todos los editores.
- La exactitud acústica para cada voz, ruido, dialecto, fórmula o vocabulario necesita audios de referencia consentidos; ninguna regla puede inferir con certeza una palabra que el ASR oyó distinta.
- El instalador no está firmado digitalmente. El SHA-256 permite verificar integridad, pero no sustituye una firma de editor.
- ARM64/32 bits no están soportados por esta entrega x64.

## Criterio de entrega

La versión se considera lista para prueba del usuario cuando el ejecutable e instalador 0.5.9 terminen de empaquetarse, sus hashes coincidan y el ejecutable de producción abra con un perfil aislado. Instalar y validar en una máquina limpia sigue siendo una prueba externa independiente.
