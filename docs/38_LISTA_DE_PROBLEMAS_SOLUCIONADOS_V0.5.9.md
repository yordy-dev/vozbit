# Vozbit 0.5.9 — problemas solucionados

Los 500 escenarios auditados eran situaciones de prueba, no 500 errores confirmados. Las correcciones se agruparon por causa para resolver varios escenarios con cada control transversal.

| Antes | Solución aplicada | Comprobación |
|---|---|---|
| El texto podía perder su destino si el foco, la ventana o el cursor cambiaban durante el procesamiento. | La entrega ahora identifica y vuelve a validar el mismo control editable mediante UI Automation; admite controles `Edit` y `Document` y conserva el resultado si el destino ya no es seguro. | Prueba unitaria de controles y smoke de producción. |
| Los errores de entrega eran genéricos y no explicaban por qué el texto quedó en Inicio. | Se añadieron códigos y mensajes diferenciados para destino cerrado, proceso/elemento cambiado, campo no editable, activación denegada, modificadores pulsados y otros casos. | Controles estáticos y recorrido de estados. |
| Existía riesgo de insertar contenido en campos sensibles o en un elemento distinto. | Se bloquean campos de contraseña y se vuelve a comprobar proceso, página, foco, elemento y capacidad de edición antes de insertar. | Pruebas unitarias e inspección del backend. |
| Un modo privado incompleto todavía podía dejar historial, aprendizaje o audio reprocesable. | El modo privado ahora impide guardar historial y aprender correcciones, y elimina/no conserva el último audio. | Smoke: historial bloqueado y audio temporal olvidado. |
| Borrar historial podía ser demasiado fácil o afectar favoritos sin suficiente claridad. | Se añadió doble confirmación y elección explícita entre borrar no favoritos o todo; la interfaz queda ocupada mientras termina. | Prueba aislada: crear 1, borrar 1 y obtener 0. |
| No había un respaldo integral antes de operaciones importantes. | Se añadió respaldo JSON versionado de ajustes, historial, diccionario y sugerencias. | Respaldo aislado comprobado con una entrada exacta. |
| Una corrupción de SQLite podía pasar desapercibida. | Se ejecuta `PRAGMA quick_check(1)` al arrancar y desde Diagnóstico. | Comprobación real con resultado `ok`. |
| Las pruebas destructivas podían tocar por error el perfil normal. | El smoke sólo permite respaldar o borrar cuando recibe una carpeta aislada explícita y verifica que la ruta creada pertenezca a ella; dos pruebas estáticas protegen esa barrera. | Producción confirmó `backup=null` y `clearedNonFavorites=null`; el historial normal permaneció intacto. |
| No era sencillo saber si fallaban disco, permisos, RAM, caché o la base. | Se creó Diagnóstico con integridad de base, escritura de datos/caché, Windows, arquitectura, CPU, RAM y espacio disponible. | Diagnóstico nativo con todos los controles locales en `ok`. |
| El diagnóstico podía exponer contenido privado. | La exportación sólo contiene información técnica; excluye audio y transcripciones. | Exportación creada y revisada en un perfil aislado. |
| Una grabación podía empezar sin espacio temporal suficiente y fallar tarde. | Se añadió una comprobación previa de espacio antes de grabar o reprocesar. | Control estático y diagnóstico de espacio real. |
| WAV y archivos auxiliares antiguos podían acumularse. | Se eliminan temporales de transcripción, precarga y salidas de motores con más de seis horas. | Control estático y diagnóstico de caché. |
| Un modelo o script reemplazado/corrupto podía ejecutarse sin detección suficiente. | Se mantienen hashes de Whisper, Parakeet y VAD y se añadieron SHA-256 para los scripts de aprendizaje. | Integridad de motores y VAD verificada en producción. |
| El cierre podía dejar grabación, transcripción, observación o procesos auxiliares activos. | Se implementó una ruta de cierre ordenado tanto desde la bandeja como al terminar Tauri. | Controles estáticos y cierre sin procesos residuales. |
| Cambiar el atajo podía liberar el anterior antes de saber si el nuevo funcionaba. | El registro es transaccional: primero prueba el nuevo y sólo después libera el anterior; la cancelación global permanece separada. | Atajo global registrado durante el smoke. |
| La lista de micrófonos podía quedar obsoleta tras conectar o retirar un dispositivo. | Los dispositivos se vuelven a enumerar cuando la app recupera el foco y se muestran errores accionables si falta el micrófono. | Compilación, controles estáticos y recorrido de configuración. |
| El remuestreo o el corte de audio podía cambiar duración o cortar habla continua. | Se reforzó el remuestreo a 16 kHz, la atenuación sobre Nyquist, la segmentación únicamente tras una pausa real y la cancelación limpia. | Cinco pruebas de audio, remuestreo y segmentación aprobadas. |
| Reglas matemáticas agresivas podían cambiar frases normales, cifras o coeficientes. | Las conversiones son deterministas y contextuales: ecuaciones, potencias, raíces, fracciones, logaritmos, RUC y puntuación hablada; nunca completa una cifra o fórmula ausente. | 14 pruebas de formateo y 9 pruebas de correcciones. |
| El autodiccionario podía aprender números, fórmulas o correcciones ambiguas como si fueran palabras. | El aprendizaje automático se limita a reemplazos claros de una palabra; números ambiguos requieren revisión, matemáticas no sesgan el prompt y reglas manuales tienen prioridad. | Cinco pruebas específicas de aprendizaje. |
| La observación de una corrección podía quedar activa demasiado tiempo o funcionar en privado. | El periodo se fijó en 30 segundos y se bloqueó completamente en modo privado. | Pruebas estáticas y de almacenamiento. |
| El indicador podía permanecer visible o no diferenciar correctamente cada estado. | Se verificaron reposo oculto, grabación verde, procesamiento naranja animado, error rojo y cancelación sin error. | Smoke nativo de los cuatro estados. |
| El tutorial podía tapar elementos o salirse de la ventana compacta. | Se ajustaron cinco pasos con tarjetas de 318 px, todas dentro del viewport y sin solapar su objetivo. | Smoke tutorial: cinco pasos, solapamiento `0`. |
| Configuración e historial podían desbordarse o degradarse con muchos registros. | Se organizaron seis categorías sin desbordamiento horizontal y se optimizó el historial visual hasta 500 entradas. | Smoke de interfaz y compilación frontend. |
| Los smoke tests sólo reconocían una URL y daban falsos negativos según el modo de ejecución. | Ahora aceptan tanto `tauri.localhost` como las URLs locales de desarrollo. | Smoke ejecutado sobre el binario empacado. |
| La entrega no tenía una verificación reproducible de versión e integridad. | Se unificó la versión 0.5.9 y se publicó `SHA256SUMS-v0.5.9.txt` para ejecutable, instalador y modelos. | Hashes recalculados y comparados; metadatos muestran Vozbit 0.5.9. |

## Evidencia final

- Frontend TypeScript/Vite: aprobado.
- Controles estáticos de endurecimiento: 20 aprobados.
- Pruebas PowerShell de correcciones: 9 aprobadas.
- Backend y formateador Rust: 42 aprobadas, 0 fallidas.
- Auditoría npm: 0 vulnerabilidades reportadas.
- Smoke del ejecutable real: motores, VAD, atajo, tutorial, configuración, privacidad, diagnóstico e indicador aprobados.

## Lo que sigue siendo una validación externa

No se presenta como resuelto sin probarlo físicamente: matriz completa de Windows 10/11, DPI y multimonitor; todos los editores externos; micrófonos USB/Bluetooth; precisión por voz y ruido; RDP/Citrix; y firma digital del instalador.
