# Matriz de cobertura de los 500 escenarios — Vozbit 0.5.9

Los documentos 33, 34 y 35 contienen 500 situaciones de prueba, no 500 errores confirmados. Esta matriz agrupa las soluciones transversales aplicadas y evita afirmar compatibilidad donde falta una máquina, dispositivo, editor o credencial externa.

| Área auditada | Cobertura transversal en 0.5.9 | Evidencia local | Límite externo o deliberado |
|---|---|---|---|
| Primer uso y comprensión | Tutorial de cinco pasos, ayuda, estado inicial, selector de micrófono/motor y mensajes accionables | Recorrido nativo completo, sin solapamientos | Prueba con usuarios no técnicos y tecnologías de asistencia |
| Ventana, bandeja e instancia | Tamaño compacto, posición conservada, cierre a bandeja, salida ordenada e instancia única | Smoke de WebView y pruebas estáticas | Suspensión, cambio de sesión y políticas corporativas requieren matriz Windows |
| Atajo global | Registro transaccional: el nuevo atajo se prueba antes de liberar el anterior; cancelación global separada | Backend inspeccionado y atajo registrado en runtime | Conflictos con utilidades externas, RDP y Sticky Keys se prueban en el entorno real |
| Micrófono y captura | Dispositivo automático, refresco al recuperar foco, prueba de entrada, re-muestreo y error visible ante caída/silencio | Pruebas de audio/resample e indicador | Bluetooth, USB desconectado, Audio Service, micrófonos multicanal y ruido físico |
| Motores locales | Parakeet y Whisper incluidos, SHA-256 de modelos/motores/VAD, precarga y cancelación | Integridad de ambos motores verificada en producción | CPU antiguas y rendimiento por hardware; no se promete ARM64/32 bits |
| Rendimiento y recursos | Precarga existente, espacio mínimo antes de grabar/reprocesar, diagnóstico de RAM/disco/CPU y temporales antiguos | Diagnóstico aislado con RAM y espacio reales | Benchmark de latencia en PCs lentas y carga simultánea externa |
| Texto, números y matemáticas | Reglas deterministas con pruebas positivas y negativas; no inventa coeficientes ni completa fórmulas ambiguas | 14 pruebas del formateador y 9 casos de correcciones | Ningún ASR garantiza 100 %; falta corpus consentido por voz/ruido/dialecto |
| Entrega al cursor | UI Automation prioritaria, controles Edit/Document, revalidación del foco, espera de modificadores y códigos estables | Test unitario y smoke de estados | Apps elevadas, canvas/Monaco, Citrix y editores que no exponen accesibilidad |
| Seguridad de entrega | Bloqueo de contraseñas, nunca envía Enter/Tab, no pega en destino dudoso y conserva el resultado para copiar | Inspección y códigos de error | Fallback por portapapeles no es automático porque expondría o reemplazaría formatos del usuario |
| Indicador | Solo aparece durante el flujo, onda verde al grabar, naranja animada al procesar, error rojo y cancelación limpia | Smoke grabando/procesando/error/cancelación | Narrador, alto contraste, DPI 125–200 % y multimonitor requieren equipos reales |
| Historial | SQLite transaccional, favoritos, límite, búsqueda parametrizada, limpieza con doble confirmación y lista optimizada hasta 500 | Prueba aislada: crear 1, respaldar 1, eliminar 1 y consultar 0 | Cifrado DPAPI y política empresarial administrada no forman parte de esta versión |
| Privacidad | Modo privado: sin historial, aprendizaje ni último audio; temporales huérfanos se limpian; reuniones siguen fuera de alcance | Smoke aislado y controles estáticos | Consentimiento legal y seguridad del perfil de Windows son externos |
| Diccionario y aprendizaje | Reglas manuales prioritarias, números ambiguos a revisión, observación de 30 s y scripts PowerShell con hash | 5 pruebas de aprendizaje y SHA verificado | La edición en apps sin UI Automation no puede observarse con fiabilidad |
| Base y recuperación | `quick_check` al arrancar y bajo demanda, respaldo JSON versionado y copia de seguridad usada con éxito en recuperación | Comparación de recuperación 20/20, sin faltantes/extras | La restauración automática no se ejecuta: sobrescribir datos requiere una decisión explícita del usuario |
| Soporte | Informe JSON con versión, Windows, RAM, disco, CPU y códigos; excluye audio y texto | Exportación dentro del perfil aislado | Compartir el archivo sigue siendo una acción explícita del usuario |
| Distribución | Instalador x64 por usuario, WebView2 offline, modelos/licencias incluidos, versión coherente y SHA-256 publicado | Build NSIS, metadatos e integridad | Firma de código exige certificado; VM limpia Windows 10/11 y revisión legal son gates externos |

## Decisiones de seguridad que se mantienen

- Vozbit no adivina una fórmula, palabra o cifra ausente.
- No usa pegado por portapapeles como fallback silencioso.
- No inserta si el campo dejó de ser editable, cambió de proceso/elemento, es una contraseña o Windows niega el foco.
- No envía Enter, Tab ni ejecuta el contenido dictado.
- No convierte el producto en grabador de reuniones.
- No añade decenas de perfiles visibles: las reglas seguras se aplican automáticamente y lo avanzado queda agrupado.

## Gates que requieren evidencia fuera de esta máquina

1. Windows 10 y 11 limpios, instalación/repair/desinstalación y actualización desde 0.5.8.
2. DPI 100/125/150/200 %, uno a tres monitores, Narrador, alto contraste y Sticky Keys.
3. Bloc de notas, Word, Excel en edición, Outlook, VS Code/Monaco, ChatGPT, Google Docs, Gmail, Teams, Slack y WhatsApp Web.
4. Micrófonos integrados, USB, Bluetooth, multicanal, desconexión y reinicio de Windows Audio.
5. Corpus de audio consentido para precisión y latencia por motor.
6. Certificado de firma y revisión de licencias para publicación pública.

Hasta completar esos gates, se consideran limitaciones documentadas y no éxitos inferidos.
